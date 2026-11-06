//! What is wrong with the file the cursor is in.
//!
//! Checks are registered in a list and iterated, never dispatched from a `match`: adding one is a
//! file plus a single line here, so two people adding two checks are not editing the same code.
//!
//! # Precision before coverage
//!
//! A checker is judged by its false positives, not by what it catches. One wrong report teaches a
//! person to stop reading the list, and after that the true ones are invisible too. So every check
//! here reports only what the index can *prove*, and everything it cannot account for -- an
//! unresolved parent, a receiver with no type, a class that answers unknown names at runtime -- is
//! silence rather than a guess.
//!
//! `just coverage` is how that is kept honest: it counts, over a real project, how much of it the
//! index actually understands. A check written against a layer that fails to type a quarter of the
//! code reports a quarter of the project as broken.

// All `pub(crate)`, because a code action names the diagnostic it repairs by the check's own
// constant rather than by a string written out a second time -- see `CodeAction::fixes`. A private
// module would leave the only way to link the two being to spell the code again, which is the link
// a typo silently breaks.
pub(crate) mod arguments;
pub(crate) mod assignment;
pub(crate) mod assumed;
pub(crate) mod callee;
pub(crate) mod catalog;
pub(crate) mod collision;
pub(crate) mod context;
pub(crate) mod deprecated;
pub(crate) mod discarded;
pub(crate) mod docblocks;
pub(crate) mod early_reads;
pub(crate) mod foreach;
pub(crate) mod function;
pub(crate) mod implementations;
pub(crate) mod imports;
pub(crate) mod impossible;
pub(crate) mod inputs;
pub(crate) mod matches;
pub(crate) mod member;
pub(crate) mod misspelt_tags;
pub(crate) mod name;
pub(crate) mod named_arguments;
pub(crate) mod nullable;
pub(crate) mod object_member;
pub(crate) mod offsets;
pub(crate) mod overrides;
pub(crate) mod placeholders;
pub(crate) mod psr4;
pub(crate) mod readonly;
pub(crate) mod receiver;
pub(crate) mod redeclared;
pub(crate) mod refused;
pub(crate) mod returned;
pub(crate) mod returns;
pub(crate) mod sealed;
pub(crate) mod service_locator;
pub(crate) mod subscripts;
pub(crate) mod syntax;
pub(crate) mod template;
pub(crate) mod twice;
pub(crate) mod unbound;
pub(crate) mod unimplemented;
pub(crate) mod unreachable;
pub(crate) mod unused;
pub(crate) mod visibility;

use phrost_model::TextRange;
use phrost_view::IndexView;

/// One problem, at one place.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    /// What the editor underlines. The name alone, not the statement around it.
    pub range: TextRange,
    pub severity: Severity,
    /// Stable identifier for the check, for suppression and for a client's "learn more".
    pub code: &'static str,
    pub message: String,
    /// Whether the finding is "this is not needed" rather than "this is wrong".
    ///
    /// The protocol's `unnecessary` tag, which editors render as faded text instead of a squiggle.
    /// The difference is not cosmetic: an unused variable is not something to look at, it is
    /// something to delete, and a squiggle asks for the wrong reaction.
    pub unnecessary: bool,
    /// Whether the finding is "this still works and is going away".
    ///
    /// The protocol's `deprecated` tag, which editors render struck through. Kept apart from
    /// [`Self::unnecessary`] for the same reason that one is kept apart from a severity: the three
    /// ask for different reactions, and a squiggle asks for the wrong one here.
    pub deprecated: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Error,
    Warning,
    /// Nothing is wrong and the checker is saying what it did.
    ///
    /// `INFORMATION` on the wire and not `HINT`, which is the level this reached for first and the
    /// level a real client does not show: Zed publishes hints inline at most and keeps them out of
    /// the diagnostics panel, so a disclosure a project *switched on to read* arrived and was
    /// invisible. A severity is a contract with the client about where a thing appears, and the
    /// quietest one available is not the right answer when the whole point is being seen.
    Note,
}

/// One thing a file can be checked for.
///
/// A trait rather than a function so a check can carry its own configuration later without
/// every other check's signature changing.
trait Check: Sync {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>);

    /// Whether the check needs the file to be whole, and not merely whole above its own finding.
    ///
    /// A check that reports the *absence* of something -- a variable nothing ever reads -- looks for
    /// evidence across the entire file, so text the parser skipped counts as evidence that is not
    /// there. Position cannot save such a check: the read that would clear the finding may be below
    /// the break. Everything else asks about what *is* written, and text above the break is written
    /// exactly as it reads.
    ///
    /// Measured rather than reasoned: breaking 399 real files one at a time, by deleting one `;`
    /// near the middle of each, fabricated exactly one finding above the break -- and it was this
    /// shape. With this exception the same sweep fabricates nothing.
    fn needs_a_whole_file(&self) -> bool {
        false
    }

    /// The code a project has to name before this check runs, when it is one of those.
    ///
    /// `None` means the check states a fact about the code and is always on. Two different reasons
    /// put a check on the other side of this, and they are worth keeping apart:
    ///
    /// - A rule about *how code is written* is a house standard. Drupal's requires a full docblock
    ///   on every function and most projects do not, and a checker that decided that for a project
    ///   would be wrong for most of them.
    /// - A fact the index cannot yet state without a known blind spot. Precision before coverage
    ///   means such a check waits rather than spending the trust the list runs on.
    ///   `undefined-function` was here until conditional declarations were indexed, which is the
    ///   way off this side: close the blind spot in the layer that has it, then turn the check on.
    fn asked_for(&self) -> Option<&'static str> {
        None
    }

    /// The check's own name, for a measurement that has to say which one a cost belongs to.
    fn label(&self) -> &'static str {
        let path = std::any::type_name::<Self>();
        path.rsplit("::").next().unwrap_or(path)
    }
}

/// What one check cost on one file.
#[derive(Clone, Debug)]
pub struct CheckCost {
    pub check: &'static str,
    pub spent: std::time::Duration,
}

/// Every check run once over the file, each timed on its own.
///
/// The instrument behind `phrost perf`'s breakdown of the worst diagnostics file: a p95 says a file
/// costs 16 ms and nothing about which of sixty checks spent it, and every check shares one view,
/// so what a check pays here includes the resolutions it is first to ask for. Read the order as
/// which check *reached* the cost, not always which one caused it.
///
/// Only the checks `asked` runs, so the sum is the file's diagnostics and not a larger number made
/// of checks nobody switched on.
pub fn check_costs(view: &IndexView<'_>, asked: AskedFor<'_>) -> Vec<CheckCost> {
    let mut out = Vec::new();
    let mut costs = Vec::with_capacity(SYNTAX.len() + SEMANTIC.len());
    for check in SYNTAX.iter().chain(SEMANTIC) {
        if !asked.wants(*check) {
            continue;
        }
        let started = std::time::Instant::now();
        check.run(view, &mut out);
        costs.push(CheckCost {
            check: check.label(),
            spent: started.elapsed(),
        });
    }
    costs
}

/// Which of the checks a project asked for beyond the ones that are always on.
///
/// By code, and the same codes a report shows, because that is what a person has in front of them
/// when they decide to switch one on. Shared with the syntax rules' own list in `phrost.toml` for
/// the same reason: a person naming a rule does not care which engine implements it.
#[derive(Clone, Copy, Default, Debug)]
pub struct AskedFor<'a> {
    pub codes: &'a [String],
    /// Codes the project switched off, which no finding here is reported under.
    ///
    /// The same `lint.off` the syntax rules read. Without it a check that is on by default could be
    /// switched off by nobody, and a project that disagrees with one rule had to choose between the
    /// rule and the rest of the checker.
    pub off: &'a [String],
}

impl AskedFor<'_> {
    fn wants(&self, check: &dyn Check) -> bool {
        match check.asked_for() {
            None => true,
            Some(code) => self.codes.iter().any(|asked| asked == code),
        }
    }

    /// Whether a finding with this code is one the project sees: every code but the ones a check
    /// waits to be asked for, and those only when asked.
    ///
    /// What `source.fixAll` asks before applying a fix on save: a fix for a finding the project
    /// never switched on is a menu entry, not an edit a save makes to a file nobody is reading.
    pub(crate) fn reports(&self, code: &str) -> bool {
        if self.switched_off(code) {
            return false;
        }
        let opted_in = SYNTAX
            .iter()
            .chain(SEMANTIC)
            .any(|check| check.asked_for() == Some(code));
        !opted_in || self.codes.iter().any(|asked| asked == code)
    }

    pub(crate) fn switched_off(&self, code: &str) -> bool {
        self.off.iter().any(|off| off == code)
    }
}

/// Checks about the text itself.
const SYNTAX: &[&dyn Check] = &[&syntax::SyntaxProblems];

/// Checks about what the code means. One line per check, and that line is the only shared edit
/// adding one needs.
const SEMANTIC: &[&dyn Check] = &[
    &arguments::ExtraArgument,
    &arguments::IncompatibleArgument,
    &arguments::WrittenBackArgument,
    &arguments::MissingArgument,
    &docblocks::MissingDocTags,
    &docblocks::WrongDocParam,
    &docblocks::UndocumentedThrow,
    &function::UndefinedFunction,
    &arguments::SentinelArgument,
    &arguments::UninstantiableNew,
    &assignment::IncompatibleAssignment,
    &assignment::SentinelAssignment,
    &catalog::UnknownCatalogKey,
    &inputs::UndefinedInput,
    &early_reads::ReadBeforeAssignment,
    &implementations::MisnamedImplementation,
    &implementations::UndeclaredImplementation,
    &assumed::AssumedNonNull,
    &callee::NotCallableValue,
    &foreach::NotIterableLoop,
    &imports::UnusedImport,
    &matches::UnhandledCase,
    &member::UndefinedMember,
    &object_member::MemberOnObject,
    &named_arguments::MisnamedArgument,
    &name::UndefinedType,
    &nullable::NullableReceiver,
    &overrides::BadOverride,
    &overrides::NothingToOverride,
    &placeholders::PlaceholderMismatch,
    &psr4::Psr4,
    &receiver::NonObjectReceiver,
    &returns::IncompatibleReturn,
    &returns::MissingReturn,
    &returns::NullableReturn,
    &subscripts::MisspeltShapeKey,
    &subscripts::PossiblyAbsentKey,
    &subscripts::ImpossibleKey,
    &subscripts::NonArraySubscript,
    &subscripts::NullableSubscript,
    &unbound::UnboundLocal,
    &unimplemented::UnimplementedMethod,
    &unreachable::UnreachableCode,
    &unused::UnusedLocal,
    &unused::UnusedPrivateMember,
    &unused::UnusedDeclaration,
    &deprecated::DeprecatedUse,
    &discarded::DiscardedResult,
    &docblocks::RedundantDocType,
    &misspelt_tags::MisspeltDocTag,
    &readonly::ReadonlyWrite,
    &redeclared::RedeclaredMember,
    &refused::RefusedDeclaration,
    &returned::RefusedReturn,
    &collision::TraitCollision,
    &context::OutOfContext,
    &impossible::ImpossibleUse,
    &offsets::BadStringOffset,
    &sealed::SealedParent,
    &twice::TwiceDeclared,
    &visibility::InaccessibleMember,
    &service_locator::ServiceLocatorCall,
];

/// Checks the file this view is about.
///
/// Syntax first, and it draws a line across the file. PHP parsing is error-tolerant, which is what
/// lets an editor answer about a file mid-edit -- but recovery skips whatever it has to, so below
/// the first break the tree is missing pieces and a semantic finding there is about a file nobody
/// wrote. Above the break the text parsed exactly as written, and a finding is worth as much as in
/// a file that parses.
///
/// That line is why the rule is positional rather than a whole-file gate. A person types at one
/// place, so the break is where the cursor is and the rest of the file is finished code -- and
/// suppressing everything would mean the longer the file, the less of it is checked while it is
/// being worked on.
///
/// The exception is a check that reports something's absence, which position cannot make safe --
/// see [`Check::needs_a_whole_file`].
///
/// Ordered by position so a client's list reads top to bottom and two runs over one file agree.
pub fn diagnostics(view: &IndexView<'_>) -> Vec<Diagnostic> {
    diagnostics_asked_for(view, AskedFor::default())
}

/// The same, with the checks a project switched on beyond the default set.
///
/// A separate entry point rather than a parameter on [`diagnostics`], so that the thirty callers
/// asking "what is wrong with this file" keep asking exactly that.
pub fn diagnostics_asked_for(view: &IndexView<'_>, asked: AskedFor<'_>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for check in SYNTAX {
        check.run(view, &mut out);
    }
    let broke_at = out.iter().map(|diagnostic| diagnostic.range.start()).min();
    // A template is read as the PHP its dialect made of it, and where a construct could not be
    // written in place -- a statement ending the file, owed a `;` with no byte left to hold it --
    // the parser's complaint is about that PHP, not about anything somebody wrote. Where it broke
    // still bounds what the rest is believed about.
    if view.is_template() {
        out.clear();
    }

    let mut semantic = Vec::new();
    for check in SEMANTIC {
        if broke_at.is_some() && check.needs_a_whole_file() {
            continue;
        }
        if !asked.wants(*check) {
            continue;
        }
        check.run(view, &mut semantic);
    }
    if let Some(at) = broke_at {
        // Strictly above, by the whole range: a finding that reaches across the break is partly
        // describing the text recovery threw away.
        semantic.retain(|diagnostic| diagnostic.range.end() <= at);
    }
    out.append(&mut semantic);
    out.retain(|diagnostic| !asked.switched_off(diagnostic.code));

    out.sort_by_key(|diagnostic| diagnostic.range.start());
    out
}

/// What is wrong with a configuration file, which is not PHP and holds no declarations.
///
/// A framework declares its services, routes and permissions in files like these and reads them
/// from PHP, so a key nothing registers is the same mistake either way -- and it is the mistake
/// nothing else catches, because the value is a string in a file no compiler reads. The judgement is
/// [`catalog::reported`], shared with the PHP path so the two cannot come to disagree about which
/// misses are worth reporting.
///
/// Nothing else yet. A class a `class:` names is checkable too -- 280 of 280 resolve on one real
/// project, so the report would be right and silent -- and is held back until a checkout that has a
/// module configured and not installed has been measured.
pub fn config_diagnostics(view: &IndexView<'_>, file_name: &str, text: &str) -> Vec<Diagnostic> {
    view.config_problems(file_name, text)
        .into_iter()
        .filter_map(|problem| {
            let mut found = catalog::reported(&problem.used)?;
            // The range comes from the walk over this file's text rather than from the index, which
            // holds nothing about it.
            found.range = phrost_model::TextRange::new(
                phrost_model::TextSize::try_from(problem.range.start).ok()?,
                phrost_model::TextSize::try_from(problem.range.end).ok()?,
            );
            Some(found)
        })
        .collect()
}
