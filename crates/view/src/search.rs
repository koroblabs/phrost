//! Finding a declaration by name, anywhere in the project.

use std::cmp::Reverse;

use phrost_index::{ClassSkeleton, Declarer};
use phrost_model::{FileId, SourceOrigin, letters_of, match_score, short_name};
use phrost_sema::MemberRow;

use crate::{
    location::{MemberModifiers, SymbolLocation},
    view::{IndexView, member_symbol_kind},
};

/// How many characters a query has to be given before names are matched loosely.
///
/// Completion's floor, for completion's reason: a project holds tens of thousands of names and `re`
/// is a subsequence of a great many of them, so below three letters the ordering constraint says
/// nothing and only a prefix is worth reading.
const LOOSE_FLOOR: usize = 3;

/// How a name matched the query.
///
/// Prefix hits ahead of loose ones, and the loose ones by how well they fit -- the same order
/// completion ranks a name page in, because it is the same question asked of the same names: what
/// begins with what was typed is what a person usually means, and `AppMan` for `OrderManager`
/// is what they mean when nothing does.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Fit {
    Prefix,
    /// Reversed so that a higher [`match_score`] sorts first.
    Loose(Reverse<u32>),
}

/// One thing the query matched, before it is worth building a location for.
///
/// Borrows rather than owns, and carries only what ranking needs. A one-letter query matches
/// thousands of names on a real project, and rendering a signature for every one of them and then
/// discarding all but a page of the result is most of what this request used to cost.
struct Match<'a> {
    /// Which tier of code this came from -- see [`origin_rank`].
    tier: u8,
    fit: Fit,
    /// What the query was compared against: a declaration's short name, or a member's own.
    name: &'a str,
    /// The declaration's qualified name, or a member's declaring class.
    qualified: &'a str,
    file: FileId,
    what: What<'a>,
}

/// What a match points at. A member is held as a borrow because it has no other identity: the
/// catalog knows top-level names, and a member is only ever reached through the class that
/// declares it.
enum What<'a> {
    Declared(Declarer),
    Member {
        class: &'a ClassSkeleton,
        row: &'a MemberRow,
    },
}

impl Match<'_> {
    /// Everything about rank that can be read without touching the file table.
    ///
    /// Origin decides before anything else: a project's own `loadByItemNumber` buried under two
    /// hundred framework hits is worse than no answer at all, because it reads as having worked.
    /// Then how the name matched -- see [`Fit`]. Then the *closest* match, measured on the name the
    /// query was typed against -- so an exact `load` comes before `LoadClassMetadataEventArgs`, and
    /// ranking by the qualified length would instead reward a shallow namespace. The name itself
    /// groups the several declarations of one thing together, which is how a reader scans the list.
    fn key(&self) -> (u8, Fit, usize, &str, &str) {
        (
            self.tier,
            self.fit,
            self.name.len(),
            self.name,
            self.qualified,
        )
    }
}

impl<'a> IndexView<'a> {
    /// Declarations whose name matches `query`, project-wide, best first.
    ///
    /// By prefix first, which is what the catalog is ordered by -- so that half is a range scan
    /// rather than a walk over every name in the project -- and then, from three characters on, by
    /// the same subsequence test completion uses: `AppMan` finds `OrderManager`, and a method
    /// is found by its humps. The loose pass over the catalog is affordable for the reason
    /// completion's is: the letter mask kept beside each name rejects most of the project with one
    /// `&` before anything reads a string. The members of the project's own classes have no mask
    /// and are scored directly, which is a few thousand short strings.
    ///
    /// Two halves, because the index stores the two differently. Classes, interfaces, traits,
    /// enums, free functions and global constants are top-level names the catalog holds by short
    /// name. Methods, properties, class constants and enum cases are not names at all as far as the
    /// catalog is concerned -- they belong to a class -- so they are found by walking the classes
    /// the project itself declares. See [`Self::collect_members`] for why that walk stops at the
    /// project's own code.
    ///
    /// `limit` is applied **after** ranking, never during collection: the protocol has no way to
    /// say a symbol search was truncated the way completion's `isIncomplete` does, so the only
    /// honest thing a cap can do is make the page it returns the best page rather than the first
    /// one found.
    pub fn search_symbols(&self, query: &str, limit: usize) -> Vec<SymbolLocation> {
        if query.is_empty() {
            return Vec::new();
        }

        let prefix = query.to_ascii_lowercase();
        let mut found = Vec::new();
        self.collect_declarations(&prefix, &mut found);
        if prefix.len() >= LOOSE_FLOOR {
            self.collect_loose_declarations(&prefix, &mut found);
        }
        self.collect_members(&prefix, &mut found);

        // The page first, then order it. A one-letter query matches several thousand names, and
        // sorting all of them to show a hundred and twenty-eight is the sort doing thirty times the
        // work the answer needs. Partitioning is linear and the sort that follows is over the page.
        if found.len() > limit {
            found.select_nth_unstable_by(limit, |left, right| self.compare(left, right));
            found.truncate(limit);
        }
        found.sort_by(|left, right| self.compare(left, right));

        found
            .into_iter()
            .filter_map(|hit| self.locate(&hit))
            .collect()
    }

    /// Best first, with the path only consulted when nothing else separates two hits.
    ///
    /// The path is what keeps the order total -- two files can declare the same name, and ranking
    /// them by `FileId` would make the answer depend on the order files were discovered, which is
    /// the one thing an index built by applying edits must not disagree with a fresh one about. It
    /// is read last because reading it means a lookup in the file table, and a tie is rare while a
    /// comparison is not.
    fn compare(&self, left: &Match<'a>, right: &Match<'a>) -> std::cmp::Ordering {
        left.key().cmp(&right.key()).then_with(|| {
            let path = |file| self.skeleton.display_path_of(file).unwrap_or_default();
            path(left.file).cmp(path(right.file))
        })
    }

    /// The short name of a located symbol, which is what a client shows in a list.
    pub fn short_name_of(location: &SymbolLocation) -> &str {
        short_name(location.fqn.as_str())
    }

    /// Top-level names the catalog holds that begin with `prefix`: class-likes, free functions,
    /// global constants.
    fn collect_declarations(&self, prefix: &str, out: &mut Vec<Match<'a>>) {
        for (_, declarers) in self.skeleton.owners().with_short_prefix(prefix) {
            self.push_declarers(declarers, Fit::Prefix, out);
        }
    }

    /// Top-level names that hold `prefix` as a subsequence without beginning with it.
    ///
    /// The letter mask first, which is one `&` against a number computed when the name was claimed;
    /// what passes is scored. A name that begins with the prefix was already offered by
    /// [`Self::collect_declarations`], and every prefix hit is a subsequence of itself.
    fn collect_loose_declarations(&self, prefix: &str, out: &mut Vec<Match<'a>>) {
        for (name, declarers) in self.skeleton.owners().with_letters(letters_of(prefix)) {
            if name.as_str().starts_with(prefix) {
                continue;
            }
            let Some(score) = match_score(name.as_str(), prefix) else {
                continue;
            };
            self.push_declarers(declarers, Fit::Loose(Reverse(score)), out);
        }
    }

    /// The declarers of one short name, ranked, one row per qualified name.
    ///
    /// One row per name because two files declaring the same one is ordinary -- `str_starts_with`
    /// is in the bundled stubs and again in symfony's polyfill -- and `rank` already decides which
    /// a reader is sent to. Two identical rows ask a person to choose between things they cannot
    /// tell apart, and they cost page slots, because the cap is applied before anything is compared.
    fn push_declarers(&self, declarers: &[Declarer], fit: Fit, out: &mut Vec<Match<'a>>) {
        let skeleton = self.skeleton;
        let mut offered: Vec<&str> = Vec::new();

        for declarer in skeleton.rank(declarers) {
            let Some(file) = skeleton.file(declarer.file) else {
                continue;
            };
            let Some(symbol) = file.scope().symbols().get(declarer.symbol as usize) else {
                continue;
            };
            if offered
                .iter()
                .any(|seen| seen.eq_ignore_ascii_case(symbol.fqn.as_str()))
            {
                continue;
            }
            offered.push(symbol.fqn.as_str());
            out.push(Match {
                tier: origin_rank(file.origin()),
                fit,
                name: short_name(symbol.fqn.as_str()),
                qualified: symbol.fqn.as_str(),
                file: declarer.file,
                what: What::Declared(declarer),
            });
        }
    }

    /// Members -- methods, properties, class constants, enum cases -- of the project's own classes.
    ///
    /// # Why the project's own code and nothing else
    ///
    /// A member has no entry in the catalog, so finding one means reading member rows. There are a
    /// quarter of a million of them on a real checkout and all but a few thousand belong to
    /// `vendor/`, so scanning them all would make a request a client sends on every character cost
    /// what the dependency tree weighs -- the one thing this index is built not to do.
    ///
    /// Refusing them costs less than it sounds. A dependency's method is something you arrive at,
    /// by goto from the call or completion on the receiver, not something you look up by name in a
    /// picker with no class beside it; and a query like `get` would spend the whole page on
    /// framework internals before reaching the first thing the person wrote. Coverage that ranks
    /// below the fold is not coverage.
    ///
    /// The classes are reached through the catalog because that is the only project-wide
    /// enumeration `phrost-view` is given. It has one useful side effect: the short-name index
    /// holds no anonymous class, so the members of one cannot turn up under a name nobody can type.
    fn collect_members(&self, prefix: &str, out: &mut Vec<Match<'a>>) {
        let skeleton = self.skeleton;
        for (_, declarers) in skeleton.owners().with_short_prefix("") {
            for declarer in declarers {
                if !declarer.kind.is_class_like() {
                    continue;
                }
                let Some(file) = skeleton.file(declarer.file) else {
                    continue;
                };
                if file.origin() != SourceOrigin::User {
                    continue;
                }
                let Some(symbol) = file.scope().symbols().get(declarer.symbol as usize) else {
                    continue;
                };
                let Some(class) = file
                    .classes()
                    .iter()
                    .find(|class| class.fqn() == &symbol.fqn)
                else {
                    continue;
                };

                for row in class.members() {
                    // A member row carries no letter mask, so the loose test is the score itself --
                    // over the project's own members only, which is a few thousand short strings.
                    let fit = if starts_with_folded(row.name.as_str(), prefix) {
                        Fit::Prefix
                    } else if prefix.len() >= LOOSE_FLOOR
                        && let Some(score) = match_score(row.name.as_str(), prefix)
                    {
                        Fit::Loose(Reverse(score))
                    } else {
                        continue;
                    };
                    out.push(Match {
                        tier: origin_rank(file.origin()),
                        fit,
                        name: row.name.as_str(),
                        qualified: class.fqn().as_str(),
                        file: declarer.file,
                        what: What::Member { class, row },
                    });
                }
            }
        }
    }

    /// The location a ranked match names, built only for the page that survived the cap.
    fn locate(&self, hit: &Match<'a>) -> Option<SymbolLocation> {
        match &hit.what {
            What::Declared(declarer) => self.best_location(&[*declarer]),
            What::Member { class, row } => Some(SymbolLocation {
                file: hit.file,
                fqn: row.name.clone(),
                kind: member_symbol_kind(row.kind),
                name_range: row.name_range,
                full_range: row.full_range,
                container: Some(class.fqn().clone()),
                from_docblock: row.from_docblock,
                signature: Some(row.signature(self.skeleton.types()).into()),
                member_modifiers: Some(MemberModifiers {
                    visibility: row.visibility,
                    set_visibility: row.set_visibility,
                    is_static: row.modifiers.is_static(),
                    is_abstract: row.modifiers.is_abstract(),
                    is_final: row.modifiers.is_final(),
                    is_readonly: row.modifiers.is_readonly(),
                }),
                // Own by construction: this walk reads each class's own member rows rather than a
                // flattened table, so nothing here arrived from a parent or a trait.
                origin: None,
                docblock_range: row.docblock_range,
                deprecated: row.modifiers.is_deprecated(),
            }),
        }
    }
}

/// Whether `name` begins with an already-folded `prefix`, ignoring ASCII case.
///
/// Over bytes rather than characters, so there is no UTF-8 boundary to land inside: the comparison
/// is byte-wise and only ASCII letters fold, which is exactly PHP's own rule for a member name.
fn starts_with_folded(name: &str, prefix: &str) -> bool {
    name.len() >= prefix.len()
        && name.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

/// Project code first, then dependencies, then the standard library.
///
/// The origin alone, which is the first three of the four tiers the index ranks conflicting
/// *declarations of one name* by. The fourth -- a dependency's test data, below even the stubs --
/// is decided by the path and lives inside `phrost-index`, out of reach from here. What that costs
/// is visible on a broad query: PHP_CodeSniffer ships lint fixtures declaring `function getVar()`,
/// and they sit among the real dependency hits instead of under them. It cannot mislead about a
/// project's own code, which outranks every one of them.
fn origin_rank(origin: SourceOrigin) -> u8 {
    match origin {
        SourceOrigin::User => 0,
        SourceOrigin::Vendor => 1,
        SourceOrigin::Stub => 2,
    }
}
