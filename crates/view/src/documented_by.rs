//! Declarations named inside a docblock, by a convention a framework stated.
//!
//! PHP has one of these of its own -- `{@inheritdoc}` -- and it is resolved by walking a hierarchy,
//! which is a different projection. This one is about a name written *as text* in a comment: the
//! docblock says which declaration carries the documentation, and the index already holds it.
//!
//! Nothing here names a framework. Which words open a claim, what a referenced name is called, and
//! whether the claim also fixes the claiming declaration's own name all arrive as
//! [`DocumentedByRule`] from the seam, and a project with no framework pays one emptiness check.
//!
//! Two questions, because two features ask them from opposite ends. Hover starts at a declaration
//! and wants the name its docblock points at. Goto starts at a position inside a comment and wants
//! whatever declaration is named there. The third -- whether a claim is one the framework can
//! honour -- is here rather than in a check because the rule that answers it is the seam's data and
//! not the checker's knowledge.

use std::cmp::Ordering;

use phrost_index::{Declarer, DocumentedByRule};
use phrost_model::{Name, SymbolKind, TextRange, TextSize};

use crate::view::IndexView;

/// A declaration whose docblock claims to implement another, and whether the claim can be honoured.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocumentationClaim {
    /// The claiming declaration's own name, and where it is written.
    pub name: Name,
    pub name_range: TextRange,
    /// What the docblock says it implements, as written.
    ///
    /// Where it is written is not carried, and deliberately: the name in the comment is already a
    /// cursor target of its own, so anything that wants to go there asks the position, and a second
    /// range here would be the same fact kept twice.
    pub implements: Box<str>,
    /// Whether anything could reach this declaration by the name the claim implies.
    ///
    /// `true` whenever the convention says nothing about naming, and whenever the answer is not
    /// decidable -- a claim is only reported where it is provably unreachable, never where the
    /// evidence merely ran out.
    pub reachable: bool,
    /// Whether the thing being implemented exists at all.
    ///
    /// Two ways it can, and both are the index answering: something declares it -- a `*.api.php`
    /// file, with the convention's placeholders applied -- or something in the project *dispatches*
    /// it, which is a declaration in every sense but the written one.
    ///
    /// The second half is what makes this reportable. Without it a project's own undocumented
    /// extension points read exactly like a hook that was removed from the framework, and on one
    /// real checkout that was six findings against one.
    pub exists: bool,
}

impl<'a> IndexView<'a> {
    /// The declaration a docblock's opening sentence says its documentation is written on.
    ///
    /// Borrowed out of the prose it was given rather than interned: the caller has the text in hand
    /// and the answer is a slice of it, so asking costs nothing on the docblocks -- the great
    /// majority -- that make no claim.
    pub fn documented_by<'p>(&self, prose: &'p str) -> Option<&'p str> {
        self.skeleton
            .returns()
            .documentation_conventions()
            .iter()
            .find_map(|rule| Some(&prose[rule.claim_in(prose)?]))
    }

    /// The declaration named at `offset` inside a docblock of the open file.
    ///
    /// Every name the convention marks, not only the claim: `@see hook_theme()` and a mention in
    /// prose are the same kind of thing, and nothing else in a comment identifies one.
    ///
    /// `None` for a name nothing declares, which is a deterministic miss rather than a reason to go
    /// looking: a docblock can name a declaration that does not exist, and answering for it would
    /// mean navigating to nowhere.
    pub fn documented_name_at(&self, offset: TextSize) -> Option<(Name, TextRange)> {
        let (written, range) = self.documented_reference_at(offset)?;
        Some((self.documented_declaration(&written)?, range))
    }

    /// The same position, answered with the calls that dispatch the name instead.
    ///
    /// The other half of [`Self::documented_name_at`], and the two are disjoint by construction: a
    /// name is either one the index holds a declaration for or one a call declares by dispatching
    /// it, and the caller asks the second only where the first said nothing. Not a fallback -- a
    /// dispatch is a different kind of answer, from a different table, and asking for it after a
    /// miss is asking a question the first table never had.
    pub fn dispatched_name_at(&self, offset: TextSize) -> Option<(Box<str>, TextRange)> {
        let (written, range) = self.documented_reference_at(offset)?;
        (!self.dispatched_declaration(&written).is_empty()).then(|| (written.into(), range))
    }

    /// The name a convention marks at `offset`, as written, and where it is written.
    ///
    /// Every name the convention marks, not only the claim: `@see hook_theme()` and a mention in
    /// prose are the same kind of thing, and nothing else in a comment identifies one.
    ///
    /// One scan with two readers above it, because two scans is how the two would come to disagree
    /// about which characters are the name -- and then goto and hover would answer about different
    /// halves of one word.
    fn documented_reference_at(&self, offset: TextSize) -> Option<(String, TextRange)> {
        let rules = self.skeleton.returns().documentation_conventions();
        if rules.is_empty() {
            return None;
        }

        let open = self.current()?;
        let docblock = open
            .hir()
            .docblocks()
            .map(|(_, data)| data)
            .find(|data| data.range.contains_inclusive(offset))?;

        for rule in rules {
            for at in rule.references_in(&docblock.text) {
                let range = shifted(&at, docblock.range.start());
                if range.contains_inclusive(offset) {
                    return Some((docblock.text[at].to_owned(), range));
                }
            }
        }
        None
    }

    /// The declaration a name written in a docblock names.
    ///
    /// Two ways a written name reaches one, and both are the index answering. Usually the name is
    /// spelled the way the declaration spells itself. Where it is not, it is the *declaration* that
    /// carries the variable part: `hook_ENTITY_TYPE_delete` is how the convention writes the name
    /// `hook_node_delete` is one of, with the uppercase run standing for the part the implementer
    /// supplies. The two are one declaration under the rule the framework stated, so reading the
    /// rule is resolution and not a second guess after a miss -- nothing is read from disk, nothing
    /// is approximate, and a written name either fills a declared name's holes exactly or does not.
    ///
    /// The rule the other direction of this file already states, with the roles swapped: there the
    /// reference carries the holes and the declaration is concrete, and
    /// [`Self::dispatch_reaches`] asks whether the framework can route between them.
    pub fn documented_declaration(&self, written: &str) -> Option<Name> {
        if let Some(key) = self.skeleton.interner().lookup_key(written)
            && let Some(declared) = self.spelling(self.skeleton.owners().function(&key))
        {
            return Some(declared);
        }

        self.skeleton
            .returns()
            .documentation_conventions()
            .iter()
            .find_map(|rule| self.holes_filled_by(rule, written))
    }

    /// The declaration whose own name `written` fills the holes in.
    ///
    /// # Why this is a lookup and not a sweep
    ///
    /// Every declaration a convention can speak for begins with the convention's own prefix, and
    /// the catalog of declared names is ordered, so the range is the hooks rather than the project:
    /// one real checkout declares 333 names beginning `hook_` out of 30 871 files, and 43 of them
    /// carry a placeholder. Everything after the range is a comparison of a handful of short
    /// strings -- the shape the return contracts use, and for the same reason.
    ///
    /// A name spelled entirely in lowercase leaves no hole, and the direct lookup above has already
    /// asked about it, so the uppercase test is both the filter and the reason a second answer here
    /// cannot contradict the first.
    ///
    /// [`PART_LIMIT`] is applied here rather than left to [`matches`], and the difference is which
    /// way silence falls. Over the limit that answers `true`, which for the reachability question is
    /// the safe direction -- an undecidable claim is not reported. Here `true` would *send a reader
    /// somewhere*, so a name past the limit is declined instead.
    fn holes_filled_by(&self, rule: &DocumentedByRule, written: &str) -> Option<Name> {
        let prefix = rule.names.as_bytes();
        if !written
            .as_bytes()
            .get(..prefix.len())?
            .eq_ignore_ascii_case(prefix)
        {
            return None;
        }
        let filled = written.get(prefix.len()..)?.split('_').collect::<Vec<_>>();
        if filled.len() > PART_LIMIT {
            return None;
        }

        let mut best: Option<(usize, Name)> = None;
        for (_, declarers) in self
            .skeleton
            .owners()
            .with_short_prefix(&rule.names.to_ascii_lowercase())
        {
            let Some(declared) = self.spelling(declarers) else {
                continue;
            };
            let text = declared.as_str();
            if !text.bytes().any(|byte| byte.is_ascii_uppercase())
                || !text
                    .as_bytes()
                    .get(..prefix.len())
                    .is_some_and(|opening| opening.eq_ignore_ascii_case(prefix))
            {
                continue;
            }

            let wanted = holes(&text[prefix.len()..]);
            let words = wanted
                .iter()
                .filter(|part| matches!(part, Part::Word(_)))
                .count();
            if words == wanted.len() || wanted.len() > PART_LIMIT || !matches(&wanted, &filled) {
                continue;
            }

            if best
                .as_ref()
                .is_none_or(|(top_words, top)| better_fit(&declared, words, top, *top_words))
            {
                best = Some((words, declared));
            }
        }
        best.map(|(_, declared)| declared)
    }

    /// How the first file declaring a function spells its name.
    ///
    /// Unranked, unlike [`Self::best_location`]: two files declaring one function name is ordinary
    /// -- a polyfill beside a stub -- but two spelling it *differently* is not, and everything this
    /// answer is handed to compares names the way PHP does, without regard to case. Ranking here
    /// would allocate a vector per candidate name for a distinction nothing downstream can see.
    fn spelling(&self, declarers: &[Declarer]) -> Option<Name> {
        let declarer = declarers
            .iter()
            .find(|declarer| declarer.kind == SymbolKind::Function)?;
        let file = self.skeleton.file(declarer.file)?;
        let symbol = file.scope().symbols().get(declarer.symbol as usize)?;
        Some(symbol.fqn.clone())
    }

    /// Every function in the open file whose docblock claims to implement something.
    ///
    /// Functions and not methods, because dispatch by name is what a claim binds and a method is not
    /// reached by its own name -- a framework that dispatches to one does it through an attribute or
    /// a registry, neither of which this could read from the name.
    pub fn documentation_claims(&self) -> Vec<DocumentationClaim> {
        let rules = self.skeleton.returns().documentation_conventions();
        if rules.is_empty() {
            return Vec::new();
        }
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        let mut found = Vec::new();
        for (_, function) in hir.functions() {
            let Some(id) = function.docblock else {
                continue;
            };
            let docblock = hir.docblock(id);
            for rule in rules {
                let Some(at) = opening_claim(rule, &docblock.text) else {
                    continue;
                };
                let implements = &docblock.text[at];
                found.push(DocumentationClaim {
                    name: function.name.clone(),
                    name_range: function.name_range,
                    implements: implements.into(),
                    reachable: self.dispatch_reaches(rule, function.name.as_str(), implements),
                    exists: self.documented_declaration(implements).is_some()
                        || self.is_dispatched(rule, implements),
                });
            }
        }

        found.sort_by_key(|claim| claim.name_range.start());
        found
    }

    /// The calls that dispatch a name a docblock references, when a convention says one could.
    ///
    /// Every convention's catalog, because the name in the comment is a name and nothing says which
    /// convention wrote it -- the same reason [`Self::documented_declaration`] tries them all.
    pub fn dispatched_declaration(&self, written: &str) -> Vec<crate::DispatchSite> {
        self.skeleton
            .returns()
            .documentation_conventions()
            .iter()
            .filter_map(|rule| rule.dispatched.as_deref())
            .flat_map(|catalog| self.dispatch_sites(catalog, written))
            .collect()
    }

    /// Whether one convention's catalog may hold the name, for a reader asking whether it exists.
    ///
    /// The looser of the two readings, deliberately -- see the dispatch projection's module doc.
    fn is_dispatched(&self, rule: &DocumentedByRule, written: &str) -> bool {
        rule.dispatched
            .as_deref()
            .is_some_and(|catalog| self.maybe_dispatched(catalog, written))
    }

    /// Whether the framework's dispatch can reach a declaration called `declared` from `implements`.
    ///
    /// The rule the seam describes: the name is an owner the `implementers` catalog holds, then the
    /// reference's own tail, with the parts joined by `_` and an uppercase run in the reference
    /// standing for one or more of them. Every way of splitting the name is tried, because the owner
    /// contains separators too -- `app_core_form_alter` is `app_core` and `form_alter`, and only the
    /// catalog says where the seam between them falls.
    ///
    /// `true` for anything not decidable. A convention that fixes no name, a reference with no tail,
    /// a name too long to split: none of those is evidence that the declaration is unreachable, and
    /// reporting on absent evidence is how a checker loses its reader.
    fn dispatch_reaches(&self, rule: &DocumentedByRule, declared: &str, implements: &str) -> bool {
        let Some(catalog) = rule.implementers.as_deref() else {
            return true;
        };
        let Some(tail) = implements
            .get(rule.names.len()..)
            .filter(|tail| !tail.is_empty())
        else {
            return true;
        };

        let wanted = holes(tail);
        for (at, _) in declared.match_indices('_') {
            if !self.holds_key(catalog, &declared[..at]) {
                continue;
            }
            if matches(&wanted, &declared[at + 1..].split('_').collect::<Vec<_>>()) {
                return true;
            }
        }
        false
    }

    /// Whether a catalog holds a key, allowing for a name written with capitals.
    ///
    /// PHP compares a function name without regard to case and a machine name is written in
    /// lowercase, so `App_Core_form_alter` is the same declaration as `app_core_form_alter` and must
    /// not be reported for spelling itself loudly.
    fn holds_key(&self, catalog: &str, key: &str) -> bool {
        let returns = self.skeleton.returns();
        returns.catalog_holds(catalog, key)
            || key.bytes().any(|byte| byte.is_ascii_uppercase())
                && returns.catalog_holds(catalog, &key.to_ascii_lowercase())
    }
}

/// One part of a reference's tail: either a word to match, or a hole standing for one or more.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part<'a> {
    Word(&'a str),
    Hole,
}

/// How many parts a name is split into before the answer stops being worth working out.
///
/// A hole backtracks over every way of splitting what follows it, so a name with several holes and
/// many parts is exponential. Real ones are nothing like it -- the longest hook a real checkout
/// declares has eight parts and two holes -- and past this the honest answer is silence.
const PART_LIMIT: usize = 24;

/// A reference's tail as parts, with each run of capitals read as a hole.
///
/// `form_FORM_ID_alter` is `form`, a hole, `alter`: the run is one placeholder however many
/// underscores its author wrote inside it, which is why the run is collapsed rather than each
/// capitalised part becoming a hole of its own. `hook_ENTITY_TYPE_delete` is otherwise two holes
/// needing two parts, and `app_core_user_delete` -- which implements it -- has one.
fn holes(tail: &str) -> Vec<Part<'_>> {
    let mut parts: Vec<Part<'_>> = Vec::new();
    for word in tail.split('_') {
        let placeholder = word.chars().any(|character| character.is_alphabetic())
            && word
                .chars()
                .all(|character| !character.is_lowercase() && !character.is_whitespace());
        match placeholder {
            true if matches!(parts.last(), Some(Part::Hole)) => {}
            true => parts.push(Part::Hole),
            false => parts.push(Part::Word(word)),
        }
    }
    parts
}

/// Whether `written` is one of the names `wanted` describes.
fn matches(wanted: &[Part<'_>], written: &[&str]) -> bool {
    if wanted.len() > PART_LIMIT || written.len() > PART_LIMIT {
        return true;
    }
    match wanted.split_first() {
        None => written.is_empty(),
        Some((Part::Hole, rest)) => {
            // At least one part, because a placeholder stands for something that was written.
            (1..=written.len()).any(|take| matches(rest, &written[take..]))
        }
        Some((Part::Word(word), rest)) => match written.split_first() {
            Some((first, tail)) if first.eq_ignore_ascii_case(word) => matches(rest, tail),
            _ => false,
        },
    }
}

/// Whether `candidate` is a closer fit than the declaration already chosen.
///
/// More of the name written out wins. A declaration spelling `form` and `alter` around its hole
/// says more about what it names than one spelling only `alter`, and the one that says more is the
/// one the writer was implementing.
///
/// Two can say exactly as much: one real checkout declares both `hook_form_FORM_ID_alter` and
/// `hook_form_BASE_FORM_ID_alter`, and by name alone they describe the same set, because which of
/// the two a form id belongs to is a fact about the form and not about the spelling. The shorter
/// placeholder is the less qualified of the pair and so the general case, which is where a reader
/// with no other information is better sent. The name itself breaks what is left, so two runs of
/// the same index agree.
fn better_fit(candidate: &Name, words: usize, chosen: &Name, chosen_words: usize) -> bool {
    words
        .cmp(&chosen_words)
        .then_with(|| chosen.len().cmp(&candidate.len()))
        .then_with(|| chosen.as_str().cmp(candidate.as_str()))
        == Ordering::Greater
}

/// The claim a docblock's opening sentence makes, as a range in the raw comment.
///
/// The *opening* and no other line: a standard asks for the sentence first, and a mention further
/// down is prose about a second declaration rather than a statement about this one.
fn opening_claim(rule: &DocumentedByRule, raw: &str) -> Option<std::ops::Range<usize>> {
    let (at, line) = undecorated(raw).find(|(_, line)| !line.trim().is_empty())?;
    let found = rule.claim_in(line)?;
    Some(at + found.start..at + found.end)
}

/// Each line of a docblock with its comment syntax removed, and where the remainder begins.
///
/// The closer is taken off before the decoration, and that order is the whole of the care needed
/// here: a line reading ` */` has its `*` eaten as decoration otherwise, leaving a `/` that reads as
/// the first line with something on it.
fn undecorated(raw: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut at = 0;
    raw.split_inclusive('\n').map(move |whole| {
        let start = at;
        at += whole.len();

        let line = whole.trim_end_matches(['\n', '\r']);
        let line = line.strip_suffix("*/").unwrap_or(line);
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();

        match trimmed.strip_prefix("/**").or_else(|| {
            trimmed
                .strip_prefix('*')
                .filter(|_| !trimmed.starts_with("*/"))
        }) {
            Some(rest) => (start + indent + (trimmed.len() - rest.len()), rest),
            None => (start + indent, trimmed),
        }
    })
}

/// A range inside a docblock, as a range inside the file.
fn shifted(at: &std::ops::Range<usize>, base: TextSize) -> TextRange {
    let start = base + TextSize::new(u32::try_from(at.start).unwrap_or(u32::MAX));
    let end = base + TextSize::new(u32::try_from(at.end).unwrap_or(u32::MAX));
    TextRange::new(start, end)
}
