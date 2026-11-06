//! What can be written here.
//!
//! Completion's raw material: names from the catalog, members from a class. Both are questions
//! about the index, so both are answered here; deciding what to *show* belongs to the feature.

use phrost_index::Declarer;
use phrost_model::{NameKey, SourceOrigin, TextSize, Visibility, letters_of, match_score};
use phrost_sema::MemberKind;

use crate::{
    completion::{Candidate, MemberCandidate, MemberDeclaration, NameKinds},
    view::IndexView,
};

/// Fills a name-completion page project code first.
///
/// Every [`SourceOrigin::User`] candidate goes in the front buffer; everything else -- vendor and
/// stubs alike -- goes in a back buffer capped at `limit`. [`Self::finish`] lays the front buffer
/// down first and only draws from the back to fill whatever room is left, so the standard library
/// (thousands of names, alphabetically ahead of most projects) can no longer fill every slot before
/// a project's own class is reached.
///
/// Two buffers instead of one sort: the walk that feeds this already visits candidates in a fixed,
/// unrelated order (short name, then the catalog's own origin-then-path tiebreak for same-named
/// declarers) and runs on every keystroke. Sorting what it finds would be `O(n log n)` over
/// however many entries a prefix matches -- thousands for a one-letter prefix. This is one linear
/// pass with two `O(1)` pushes; [`Self::is_full_for`] skips even the symbol lookup for a filler
/// entry the back buffer has no room for, so the only extra cost a non-matching prefix pays for is
/// the origin check it needed anyway.
/// How many characters a name has to be given before it is matched loosely.
///
/// A fact about the population rather than about the matcher: a project holds tens of thousands of
/// names, and `re` is a subsequence of a great many of them. Three is where the ordering constraint
/// between letters starts to say something. A class's *members* are thirty and use two -- the same
/// test, a different crowd.
const LOOSE_FLOOR: usize = 3;

struct RankedPage {
    preferred: Vec<Candidate>,
    /// What begins with the prefix, the project's own and then everything else; and after both, what
    /// holds it loosely, in the same two halves. See [`Self::loosen`].
    tiers: [Vec<Candidate>; 4],
    /// Whether the walk has moved on to loose matches, which every prefix match outranks.
    loose: bool,
    limit: usize,
}

/// Where in [`RankedPage::tiers`] a candidate goes.
const USER: usize = 0;
const OTHER: usize = 1;

impl RankedPage {
    fn new(limit: usize) -> Self {
        Self {
            preferred: Vec::new(),
            tiers: Default::default(),
            loose: false,
            limit,
        }
    }

    /// The tier a candidate of this origin goes in, at this point of the walk.
    fn tier(&self, origin: SourceOrigin) -> usize {
        let half = match origin {
            SourceOrigin::User => USER,
            _ => OTHER,
        };
        half + if self.loose { 2 } else { 0 }
    }

    /// Whether a candidate of this origin can be discarded before it is even built.
    ///
    /// True once its tier holds a page's worth: a candidate there could do nothing but be dropped by
    /// [`Self::finish`], so the file and symbol lookups that would build it are skipped. A project's
    /// own candidate is never skipped this way while its tier has room -- it outranks everything
    /// after it, wherever the walk finds it.
    fn is_full_for(&self, origin: SourceOrigin) -> bool {
        self.tiers[self.tier(origin)].len() >= self.limit
    }

    /// The fully-qualified names already on the page, for the dedup the later passes run.
    fn offered(&self) -> impl Iterator<Item = &str> {
        self.preferred
            .iter()
            .map(|candidate| candidate.fqn.as_str())
    }

    /// Whether the prefix matches alone fill the page.
    ///
    /// The gate on the loose pass: every loose match ranks after every prefix match, so with a page
    /// of those there is nothing a looser match could add, and the walk over the project's names
    /// does not happen at all.
    fn is_full(&self) -> bool {
        self.preferred.len() + self.tiers[USER].len() + self.tiers[OTHER].len() >= self.limit
    }

    /// Records a candidate ahead of every other tier.
    ///
    /// Two kinds earn it and origin enters into neither: one the place asked for by name -- at an
    /// attribute's position a framework's `#[Route]` is as much the answer as a project's own -- and
    /// one whose name is exactly what was typed.
    fn prefer(&mut self, candidate: Candidate) -> bool {
        self.preferred.push(candidate);
        self.preferred.len() >= self.limit
    }

    /// From here on what is pushed is a loose match, ranked after every prefix match.
    ///
    /// The two passes used to share one front buffer for the project's own names, so a project
    /// holding `t`, `i`, `m` and `e` in order put `ShipmentLineSegment` ahead of PHP's `time` -- and on
    /// a real checkout enough of them to push `time` off the page altogether.
    fn loosen(&mut self) {
        self.loose = true;
    }

    /// Records one candidate. Returns `true` once nothing still to come could reach the page, so the
    /// caller can stop walking: what ranks at or above the tier being filled already fills it.
    fn push(&mut self, origin: SourceOrigin, candidate: Candidate) -> bool {
        let tier = self.tier(origin);
        if self.tiers[tier].len() < self.limit {
            self.tiers[tier].push(candidate);
        }
        // The project's own half is the first of its pass, so once it and everything ranked above it
        // fill the page, the rest of the pass is dropped by `finish` whatever it holds.
        let first_of_pass = if self.loose { 2 } else { 0 };
        let above = self.preferred.len()
            + self.tiers[..=first_of_pass]
                .iter()
                .map(Vec::len)
                .sum::<usize>();
        tier == first_of_pass && above >= self.limit
    }

    /// The tiers in order, cut at `limit`.
    fn finish(self) -> Vec<Candidate> {
        let mut out = self.preferred;
        for tier in self.tiers {
            let room = self.limit.saturating_sub(out.len());
            out.extend(tier.into_iter().take(room));
        }
        out
    }
}

/// A name already on the page, as PHP tells two names apart: by which of its three tables it is in
/// and then by the name, with the case rule of that table.
///
/// One row per name is the rule, and the name alone is not a name -- `Termwind\Terminal` is a class
/// and `Termwind\terminal` a function beside it, in Laravel's own dependencies, and comparing the
/// two paths without case kept the class and dropped the function that `use function` imported.
struct OfferedName<'a> {
    fqn: &'a str,
    table: SymbolTable,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SymbolTable {
    ClassLike,
    Function,
    Constant,
}

impl<'a> OfferedName<'a> {
    fn of(fqn: &'a str, kind: phrost_model::SymbolKind) -> Self {
        Self {
            fqn,
            table: match kind {
                phrost_model::SymbolKind::Function => SymbolTable::Function,
                phrost_model::SymbolKind::Constant => SymbolTable::Constant,
                _ => SymbolTable::ClassLike,
            },
        }
    }

    fn is(&self, other: &Self) -> bool {
        self.table == other.table
            && match self.table {
                // Exactly, as PHP compares a constant's name.
                SymbolTable::Constant => self.fqn == other.fqn,
                SymbolTable::ClassLike | SymbolTable::Function => {
                    self.fqn.eq_ignore_ascii_case(other.fqn)
                }
            }
    }
}

impl<'a> IndexView<'a> {
    /// Top-level symbols whose short name starts with `prefix`, and whose kind `kinds` accepts.
    ///
    /// Case-insensitive, because completion is a suggestion and `rep` should offer `Repo`. Capped by
    /// `limit`: on a real project a one-letter prefix matches thousands of names, and a client is
    /// going to filter and show a dozen.
    ///
    /// `kinds` is applied before the cap, not after. A position that can only take a class -- `new `
    /// -- offering every function and constant the prefix matches first spends the whole budget on
    /// rows nobody could accept: measured on a real project, two hundred of them before the first
    /// class. Filtering afterward, in the caller, does not get the budget back.
    ///
    /// Project code first, then vendor and stubs: see [`RankedPage`]. Within a tier, ordered by
    /// short name, then by the same ranking resolution uses, so the copy that would actually
    /// resolve is offered first among same-named declarers.
    pub fn candidates_with_prefix(
        &self,
        offset: TextSize,
        prefix: &str,
        kinds: NameKinds,
        limit: usize,
    ) -> Vec<Candidate> {
        // A prefix with a separator in it is anchored: `\Drupal` means a name that *begins* that
        // way, not one whose last segment happens to. Matching short names there offers
        // `Drupal6SqlBaseTest` for `\Drupal`, which is not a name the person could have meant.
        //
        // Where it is anchored *to* is the def-map's answer, the same one a complete name gets: a
        // fully-qualified prefix means the root, and a relative one means whatever the file's
        // imports and namespace make of it.
        if prefix.contains('\\') {
            return self.path_continuations(offset, prefix, kinds, limit).names;
        }

        let folded = prefix.to_ascii_lowercase();
        let mut page = RankedPage::new(limit);
        let mut taken: Vec<OfferedName<'_>> = Vec::new();

        // The tier in front, and only ever one place asks for it: after a `#[`, the classes that
        // carry `#[Attribute]` are the ones that can stand there, and on a real checkout there are
        // 441 of them among 30 916 class-likes, against 21 787 places that write one.
        // Alphabetically they are nowhere near the top, so without this the page is two hundred
        // rows of things a person cannot mean -- measured, it began `AbstractClientStrategy`,
        // `AccessToken`, `AccountPermissions`, and reached no attribute at all.
        //
        // Named by the index rather than found by walking: `#[` on its own has no prefix to stop a
        // walk, so deciding this by asking each class in turn is a lookup per name in the project
        // on every keystroke. The list is a scan of a few dozen short strings, and the class's own
        // row still decides -- see `ProjectSkeleton::attribute_names`.
        if kinds == NameKinds::Attribute {
            for key in self.attribute_keys(&folded) {
                for declarer in self.skeleton.rank(self.skeleton.owners().short_named(&key)) {
                    if !kinds.accepts(declarer.kind) {
                        continue;
                    }
                    let Some(file) = self.skeleton.file(declarer.file) else {
                        continue;
                    };
                    let Some(symbol) = file.scope().symbols().get(declarer.symbol as usize) else {
                        continue;
                    };
                    // The name is only the prefilter -- a list that is only ever added to says a
                    // class *was* an attribute. What decides is its own row, asked the one way
                    // every other reader asks for it.
                    if !self.class_is_attribute(symbol.fqn.as_str()) {
                        continue;
                    }
                    if page.offered().any(|seen| seen == symbol.fqn.as_str()) {
                        continue;
                    }
                    taken.push(OfferedName::of(symbol.fqn.as_str(), symbol.kind));

                    if page.prefer(Candidate {
                        fqn: symbol.fqn.clone(),
                        kind: symbol.kind,
                        file: declarer.file,
                        deprecated: symbol.is_deprecated(),
                    }) {
                        return page.finish();
                    }
                }
            }
        }

        // Two passes over the same names, and the order between them *is* the ranking. A name is
        // usually written from its start, so what begins with what was typed comes first and the
        // cheap range scan is what finds it. But `OrderManager` is what a person means by
        // `AppMan`, and a prefix test answers nothing for it -- so what is left of the page is
        // filled by the same subsequence test a member list uses, ranked by how well each fits.
        for (name, declarers) in self.skeleton.owners().with_short_prefix(&folded) {
            // The name exactly as typed leads, wherever it is declared: `time` is PHP's function
            // before it is the start of the project's `TimeTrait`.
            let exact = name.as_str() == folded;
            // One row per name, not per declarer. Two files declaring the same name is ordinary --
            // `str_starts_with` is in the bundled stubs and again in symfony's polyfill, and a class
            // moved between packages is in both for a release -- and `rank` already decides which of
            // them a reader is sent to. Offering the loser as a second identical row asks a person
            // to choose between two things they cannot tell apart.
            //
            // Within the group, because the key is a folded short name: two *different* qualified
            // names sharing one short name are two real choices and both belong.
            let mut offered: Vec<OfferedName<'_>> = Vec::new();

            for declarer in self.skeleton.rank(declarers) {
                if !kinds.accepts(declarer.kind) {
                    continue;
                }
                let Some(file) = self.skeleton.file(declarer.file) else {
                    continue;
                };
                if !exact && page.is_full_for(file.origin()) {
                    continue;
                }
                let Some(symbol) = file.scope().symbols().get(declarer.symbol as usize) else {
                    continue;
                };
                let name = OfferedName::of(symbol.fqn.as_str(), symbol.kind);
                if offered.iter().chain(&taken).any(|seen| seen.is(&name)) {
                    continue;
                }
                offered.push(name);

                let candidate = Candidate {
                    fqn: symbol.fqn.clone(),
                    kind: symbol.kind,
                    file: declarer.file,
                    deprecated: symbol.is_deprecated(),
                };
                let full = match exact {
                    true => page.prefer(candidate),
                    false => page.push(file.origin(), candidate),
                };

                if full {
                    return page.finish();
                }
            }
            taken.extend(offered);
        }

        if folded.len() >= LOOSE_FLOOR && !page.is_full() {
            page.loosen();
            // The letter mask first, which is one `&` against a number computed when the name was
            // claimed: a name cannot hold `folded` as a subsequence unless it holds every letter of
            // it. Measured on a 30 964-file checkout, the walk itself is a tenth of a millisecond
            // and scoring every name is the rest -- so the gate is what makes the pass affordable.
            let mut scored: Vec<(u32, &NameKey, &[Declarer])> = self
                .skeleton
                .owners()
                .with_letters(letters_of(&folded))
                .filter_map(|(name, declarers)| {
                    Some((match_score(name.as_str(), &folded)?, name, declarers))
                })
                .collect();
            // Best first, then the shorter name, then alphabetically -- so a tie is settled the same
            // way on every keystroke. A list that reshuffles as somebody types is worse than a short
            // one.
            scored.sort_by(|left, right| {
                right
                    .0
                    .cmp(&left.0)
                    .then_with(|| left.1.as_str().len().cmp(&right.1.as_str().len()))
                    .then_with(|| left.1.as_str().cmp(right.1.as_str()))
            });

            for (_, _, declarers) in scored {
                for declarer in self.skeleton.rank(declarers) {
                    if !kinds.accepts(declarer.kind) {
                        continue;
                    }
                    let Some(file) = self.skeleton.file(declarer.file) else {
                        continue;
                    };
                    if page.is_full_for(file.origin()) {
                        continue;
                    }
                    let Some(symbol) = file.scope().symbols().get(declarer.symbol as usize) else {
                        continue;
                    };
                    // What the first pass already offered: every prefix hit is also a subsequence
                    // of itself, so without this each of them would be offered twice.
                    let name = OfferedName::of(symbol.fqn.as_str(), symbol.kind);
                    if taken.iter().any(|seen| seen.is(&name)) {
                        continue;
                    }
                    taken.push(name);

                    let full = page.push(
                        file.origin(),
                        Candidate {
                            fqn: symbol.fqn.clone(),
                            kind: symbol.kind,
                            file: declarer.file,
                            deprecated: symbol.is_deprecated(),
                        },
                    );
                    if full {
                        return page.finish();
                    }
                }
            }
        }

        page.finish()
    }

    /// The attribute short names a folded prefix reaches, best match first.
    ///
    /// The same two-part rule the catalog walk follows -- what begins with what was typed, then
    /// what holds it as a subsequence once enough has been typed for that to mean anything -- run
    /// over the index's list instead of over the catalog. A prefix hit outranks every loose one,
    /// and ties are settled alphabetically so that two identical sessions offer the same page.
    fn attribute_keys(&self, folded: &str) -> Vec<NameKey> {
        let mut named: Vec<(u32, &NameKey)> = self
            .skeleton
            .attribute_names()
            .iter()
            .filter_map(|key| {
                if key.as_str().starts_with(folded) {
                    return Some((u32::MAX, key));
                }
                if folded.len() < LOOSE_FLOOR {
                    return None;
                }
                match_score(key.as_str(), folded).map(|score| (score, key))
            })
            .collect();
        named.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.as_str().cmp(right.1.as_str()))
        });
        named.into_iter().map(|(_, key)| key.clone()).collect()
    }

    /// The names a walk found directly under a prefix, as a page: the kinds the place takes, the
    /// declarer ranking decides between, the project's own first.
    ///
    /// Directly under, because a qualified name's last segment is the name and the rest is the
    /// namespace: `\\Drupal\\Core\\Ent` is a name being typed in `Drupal\\Core`, so
    /// `Drupal\\Core\\Entity` belongs and `Drupal\\Core\\Entity\\Node` does not -- that is
    /// [`Self::under`]'s other half. Without the rule, `\\Drupal` matches every class in the
    /// framework.
    ///
    /// The walk is a scan of the catalog rather than an index: an ordered second copy of every name
    /// would cost more memory than a prefix search is worth, and one pass is well inside the budget.
    ///
    /// `kinds` narrows within class-like, not just to it: `new Data\\No` accepts a class but not an
    /// interface, trait or enum living under the same namespace, all of which this catalog also
    /// files as class-like. Ranked project-code-first via [`RankedPage`]: a namespace shared between
    /// a project and a dependency should offer the project's own class first.
    pub(crate) fn ranked_names(
        &self,
        found: Vec<(&str, &[phrost_index::Declarer])>,
        kinds: NameKinds,
        limit: usize,
    ) -> Vec<Candidate> {
        let mut page = RankedPage::new(limit);
        for (_, declarers) in found {
            for declarer in self.skeleton.rank(declarers) {
                if !kinds.accepts(declarer.kind) {
                    continue;
                }
                let Some(file) = self.skeleton.file(declarer.file) else {
                    continue;
                };
                if page.is_full_for(file.origin()) {
                    continue;
                }
                let Some(symbol) = file.scope().symbols().get(declarer.symbol as usize) else {
                    continue;
                };
                // The class-like table is what resolution reads, so it holds names nobody can
                // write -- an anonymous class is in it so that `$this` inside its body resolves.
                // The unqualified walk above ranges over the short-name index, which never holds
                // one; this walk ranges over the table itself, so it has to say so.
                if !symbol.is_nameable() {
                    continue;
                }
                let full = page.push(
                    file.origin(),
                    Candidate {
                        fqn: symbol.fqn.clone(),
                        kind: symbol.kind,
                        file: declarer.file,
                        deprecated: symbol.is_deprecated(),
                    },
                );
                if full {
                    return page.finish();
                }
            }
        }

        page.finish()
    }

    /// Members of a class that are reachable from `access`.
    ///
    /// `access` is the visibility the asking scope has earned: `Private` inside the declaring class,
    /// `Protected` inside a subclass, `Public` from outside. Filtering here rather than in the
    /// caller keeps every feature that offers members honest about the same rule.
    pub fn member_candidates(&self, class_fqn: &str, access: Visibility) -> Vec<MemberCandidate> {
        let table = self.members(class_fqn);
        // A macro answers on the instance, publicly, wherever no declaration of the name does --
        // the rule `member_held_on` reads it by.
        let macros = self
            .macros_answered_by(class_fqn)
            .into_iter()
            .filter(|name| table.find(MemberKind::Method, name.as_str()).is_none())
            .map(|name| MemberCandidate {
                ty: self
                    .macro_rendered(class_fqn, name.as_str())
                    .map(Into::into),
                name,
                kind: MemberKind::Method,
                visibility: Visibility::Public,
                is_static: false,
                origin: phrost_sema::MemberProvenance::Own,
                deprecated: false,
                forwarded: true,
                opened: false,
                takes_arguments: true,
                parameters: Box::default(),
            })
            .collect::<Vec<_>>();
        table
            .members()
            .iter()
            .filter(|member| member.reached_with(access))
            .map(|member| MemberCandidate {
                name: member.name.clone(),
                kind: member.row.kind,
                visibility: member.visibility,
                is_static: member.row.modifiers.is_static(),
                ty: member
                    .row
                    .rendered_type(self.skeleton.types())
                    .map(Into::into),
                origin: member.provenance.clone(),
                deprecated: member.row.modifiers.is_deprecated(),
                forwarded: member.forwarded,
                opened: member.opened,
                // A parameter with a default still counts: `find(int $id = 0)` is a call somebody
                // usually passes something to, and the cursor going inside the parentheses is right
                // either way -- there is nothing to type only when there is no parameter at all.
                takes_arguments: member.row.kind == MemberKind::Method
                    && !member.row.params.is_empty(),
                parameters: match member.row.kind {
                    MemberKind::Method => member
                        .row
                        .params
                        .iter()
                        .take_while(|param| !param.has_default && !param.is_variadic)
                        .map(|param| param.name.clone())
                        .collect(),
                    _ => Box::default(),
                },
            })
            .chain(macros)
            .collect()
    }

    /// One member's declaration as text: `public function find(int $id): ?Repo`.
    ///
    /// Its own lookup rather than a field on every candidate, because a client shows the declaration
    /// of the *one* item a person has highlighted. Rendering it for every member of the class on
    /// every keystroke was hundreds of signatures built and discarded per request, and it scaled with
    /// the size of the class rather than with what was typed.
    ///
    /// Which is also why it can afford [`Self::resolved_signature`]: one highlighted item pays for
    /// the climb that tells a reader what `{@inheritdoc}` over a bare signature comes to, and the
    /// card a client shows beside the list is then the card hover shows at the same member. Two
    /// descriptions of one declaration is the shape this repeatedly turns out to be.
    pub fn member_declaration(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<MemberDeclaration> {
        let table = self.members(class_fqn);
        let member = table.find(kind, name)?;
        Some(MemberDeclaration {
            visibility: member.visibility,
            is_static: member.row.modifiers.is_static(),
            kind: member.row.kind,
            deprecated: member.row.modifiers.is_deprecated(),
            signature: self
                // Completion holds the receiver, so a `static` return is the class it
                // was written on: `$user->` offers `load(): ?User`.
                .resolved_signature(class_fqn, kind, name, Some(class_fqn))
                .unwrap_or_else(|| member.row.signature(self.skeleton.types())),
        })
    }
}
