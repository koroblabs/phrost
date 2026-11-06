//! What can be done about what is wrong, at one place in the file.
//!
//! Offers are registered in a list and iterated, never dispatched from a `match`: adding one is a
//! file plus a single line here.
//!
//! # Pulled, not pushed
//!
//! This is the one feature a person asks for by name. Diagnostics arrive unbidden, so a wrong one
//! costs attention every time the file is saved; an action is asked for at a position and its cost
//! is one glance at a menu. That difference is worth stating because it moves the line: an offer may
//! be speculative where a report may not -- eight classes are called `Node` in a Drupal project and
//! listing all eight is the honest answer, where a diagnostic naming one of them would be a guess.
//!
//! What it does *not* move is the edit. An action that produces a file which no longer parses, or
//! one that silently changes what a name means, is worse than no action: it is a change a person did
//! not read and now has to find. So every offer here is refused rather than approximated when the
//! index cannot prove the edit is the one meant.

mod document;
mod prose;
mod subject;

mod accessors;
mod annotation;
mod constant;
mod constructor;
mod defaulted;
mod doc_tags;
mod extract;
mod fix_all;
mod implement;
mod import;
mod inheritdoc;
mod inline;
mod misspelt_tag;
mod named_label;
mod namespaced;
mod nullsafe;
mod organize_imports;
mod promote;
mod refine_return;
mod unused_import;

pub use self::document::DocStyle;

use phrost_model::{TextRange, TextSize};
use phrost_view::IndexView;

use self::subject::{ActionSubject, SubjectSites};

/// What sort of thing an offer is, in the protocol's own terms.
///
/// Flat, where the protocol's kinds are a dotted hierarchy. Translating one to the other is the
/// LSP layer's job, because this crate holds no protocol types -- and a client's filter is
/// hierarchical, so `refactor` selects every refactor without this needing to know how many there
/// are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionKind {
    /// Something the index can point at and say is wrong. Belongs beside a diagnostic.
    QuickFix,
    /// A change to code that is not wrong. Nothing is being repaired, so nothing about it is
    /// preferred and a client must never apply one unasked.
    Refactor,
    /// Organizing the file's imports. The protocol's own `source.organizeImports`, which an editor
    /// binds to a menu entry and often to saving.
    ///
    /// Named for the specific thing rather than for the `source` family it belongs to, because a
    /// client asks for it by the specific name: a general `source` would not match a request for
    /// `source.organizeImports`, and the menu entry a person is looking for would simply be absent.
    /// A second source action gets its own variant when there is one.
    OrganizeImports,
    /// Every fix in the file that needs nobody to choose, as one edit -- the protocol's
    /// `source.fixAll`, which an editor binds to saving. See `fix_all`.
    FixAll,
}

/// Everything an offer is given about one request.
///
/// A struct and not three arguments, which is the whole point: what a code action needs is going to
/// grow -- the client's capabilities, whether it supports resolving an edit lazily, which formatting
/// the project uses -- and each of those arriving as a parameter would edit every offer's signature
/// and every offer's file. One field here edits one file.
#[derive(Debug)]
pub struct ActionRequest<'a> {
    pub view: &'a IndexView<'a>,
    /// What the client selected. Empty when there is only a cursor.
    pub at: TextRange,
    /// The kinds the client asked for, already translated. Empty means it asked for everything.
    ///
    /// Honoured before an offer runs, so a client asking only for quick fixes does not pay for the
    /// refactors. It is only half of what keeps this affordable, though: an editor asking on
    /// cursor-idle asks for every kind, and what saves that request is [`subject`].
    pub only: &'a [ActionKind],
    /// The checks the project switched on beyond the ones always on -- what `source.fixAll` may
    /// apply on its own. A fix for a finding the project never asked to see is still offered in the
    /// menu; applied on save it is an edit nobody wanted, and a docblock rewritten in every file a
    /// person touches.
    pub asked: crate::AskedFor<'a>,
}

impl ActionRequest<'_> {
    /// Whether the client wants this kind at all.
    fn wants(&self, kind: ActionKind) -> bool {
        self.only.is_empty() || self.only.contains(&kind)
    }
}

/// One thing a person can have done to the file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CodeAction {
    /// What the menu says. It names the thing being changed, because a menu of four identical
    /// titles is a menu nobody can use.
    pub title: String,
    /// Applied together and non-overlapping, in the file the request was about.
    pub edits: Vec<TextEdit>,
    /// Whether a client that applies one action without asking should apply this one.
    ///
    /// True only where there is nothing to choose between: one candidate, one edit. A list of eight
    /// imports has no preferred member, and marking one would make a keystroke pick a class at
    /// random.
    pub preferred: bool,
    pub kind: ActionKind,
    /// The diagnostic this repairs, by the code the check itself declares.
    ///
    /// `None` for anything that is not repairing a finding. Present ones let a client show the fix
    /// on the squiggle rather than only in a menu -- and taking it from the check's own constant is
    /// what keeps the two from drifting apart, which a string written out in both places would not.
    pub fixes: Option<&'static str>,
}

/// A replacement of one range with one string. An empty range is an insertion.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextEdit {
    pub range: TextRange,
    pub text: String,
}

impl TextEdit {
    fn insert(at: TextSize, text: impl Into<String>) -> Self {
        Self {
            range: TextRange::empty(at),
            text: text.into(),
        }
    }

    fn replace(range: TextRange, text: impl Into<String>) -> Self {
        Self {
            range,
            text: text.into(),
        }
    }
}

/// One kind of offer.
trait Offer: Sync {
    /// Checked before [`Self::run`], so an offer the client filtered out costs nothing.
    fn kind(&self) -> ActionKind;

    /// The sort of thing this acts on, checked before [`Self::run`] against what the range holds.
    ///
    /// Required rather than defaulted: a default would be "runs always", and running always is what
    /// made six offers cost six whole-file walks on a request where one of them had anything to
    /// say. See [`subject`].
    fn subject(&self) -> ActionSubject;

    /// The sites for [`Self::subject`] are already narrowed to the range, so an offer reads them
    /// rather than walking the file to find them again.
    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>);

    /// Whether this offer's *preferred* actions may be applied to a whole file with nobody reading
    /// them first -- which is what `source.fixAll` does on save.
    ///
    /// `false` unless the offer says otherwise, because the default has to be the safe one: a fix
    /// that changes what the code does, or writes prose somebody should read, stays a menu entry.
    fn unattended(&self) -> bool {
        false
    }
}

/// One line per offer, and that line is the only shared edit adding one needs.
const OFFERS: &[&dyn Offer] = &[
    &accessors::GenerateAccessors,
    &annotation::AnnotationAsAttribute,
    &constant::ExtractConstant,
    &constructor::GenerateConstructor,
    &document::WriteDocblock,
    &doc_tags::AddMissingDocTag,
    &misspelt_tag::RenameMisspeltTag,
    &named_label::CorrectNamedLabel,
    &extract::ExtractVariable,
    &implement::ImplementRequired,
    &import::ImportClass,
    &inline::InlineVariable,
    &namespaced::FixNamespace,
    &organize_imports::OrganizeImports,
    &promote::PromoteParameter,
    &refine_return::RefineReturn,
    &inheritdoc::InheritDocblock,
    &defaulted::DefaultTheRead,
    &nullsafe::NullsafeCall,
    &unused_import::RemoveUnusedImport,
    // Last, so the menu reads specific fixes first and the whole file after them.
    &fix_all::FixAll,
];

/// Everything on offer for the range the request was about.
///
/// The range and not a position: a client sends the selection, which is empty when there is only a
/// cursor. An offer about a name overlapping it counts, so selecting a whole line offers what is on
/// that line.
pub fn code_actions(request: &ActionRequest<'_>) -> Vec<CodeAction> {
    let mut out = Vec::new();
    let sites = SubjectSites::of(request);
    for offer in OFFERS {
        // Both gates before `run`, cheapest first: what the client asked for is a slice lookup,
        // and what the range holds is at worst one projection shared by every offer that reads it.
        if !request.wants(offer.kind()) {
            continue;
        }
        if !sites.holds(offer.subject()) {
            continue;
        }
        offer.run(&sites, &mut out);
    }
    out
}

/// Whether a subject at `subject` is what a request about `at` is asking about.
///
/// Touching counts, so a cursor sitting just after a name is still on it -- which is where a cursor
/// is when somebody has just finished typing one.
fn concerns(subject: TextRange, at: TextRange) -> bool {
    subject.start() <= at.end() && at.start() <= subject.end()
}
