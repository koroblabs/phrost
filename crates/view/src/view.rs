use std::{cell::RefCell, sync::Arc};

use phrost_index::{OpenFile, OpenSet, ProjectSkeleton};
use phrost_model::{FileId, SymbolKind, TextRange, TextSize};
use phrost_sema::{MemberKind, MemberTable};
use rustc_hash::FxHashMap;

use crate::{location::SymbolLocation, resolve::Held};

/// Whether the framework contracts that take a sentinel off a declaration are in force.
///
/// Two values rather than a bool so that the reading side cannot be got the wrong way round, and
/// a type rather than a parameter on `contracted` so that lifting them is a property of the whole
/// view: a memoized resolution taken with them lifted must not be handed to anybody who asked with
/// them applied, and giving the twin its own caches is what rules that out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Contracts {
    Applied,
    Lifted,
}

/// What each path resolved to at each position, and whether a contract changed the answer.
///
/// By position and budget first and the path second, so a lookup compares the path rather than
/// cloning it into a key and hashing it: resolution asks this more than anything else, and the
/// clone per ask was a large share of what allocation cost.
pub(crate) type ResolvedPaths =
    FxHashMap<(TextSize, u8), Vec<(phrost_body::ValuePath, (Option<Held>, bool))>>;

/// Whether each statement's callee says anything about a path the statement hands it.
///
/// By statement and budget first and the path second, as [`ResolvedPaths`] is and for its reason:
/// one statement hands over a path or two, so the lookup compares rather than hashing a clone.
pub(crate) type SpeakingStatements =
    FxHashMap<(phrost_hir::StmtId, u8), Vec<(phrost_body::ValuePath, bool)>>;

/// What a call's callee takes, and the templates its receiver supplied, keyed by where it is written.
pub(crate) type CallParameters = FxHashMap<
    (phrost_model::TextRange, phrost_model::TextRange),
    Option<(
        Box<[crate::parameters::Parameter]>,
        Option<crate::hints::CalleeTemplates>,
    )>,
>;

/// Read-only projections over one index snapshot.
///
/// Cheap to construct — it borrows — so a request builds one, answers, and drops it. It reads and
/// never writes the index, so two requests can hold views at the same time.
#[derive(Debug)]
pub struct IndexView<'a> {
    pub(crate) skeleton: &'a ProjectSkeleton,
    pub(crate) open: &'a OpenSet,
    /// The file the request is about. Position queries need it; cross-file ones do not.
    current: Option<FileId>,
    /// Whether a framework's return contract is applied to the types this view hands out.
    ///
    /// Every view but one applies them; the exception is the twin
    /// [`Self::with_contracts_lifted`] builds, which exists so that "what did a contract decide"
    /// can be answered by running the ordinary checks rather than by a second rule about
    /// nullability. See [`crate::assumed_non_null`].
    pub(crate) contracts: Contracts,
    /// Flattened member tables, memoized for as long as this view lives.
    ///
    /// Scoped to one request on purpose. Flattening is pure, so caching it is safe at any lifetime;
    /// the *invalidation* is not, and a cache that outlives an edit is the bug class that made the
    /// previous attempt feel unpredictable. A view is dropped when the request is answered, so this
    /// one cannot go stale — there is no edit it can survive.
    ///
    /// It pays for itself inside a single request rather than across them, which is where the cost
    /// actually is: one pass over a file asks about `$this` hundreds of times, and every ask used to
    /// re-walk the whole hierarchy above it. The skeleton keeps a second, bounded copy across
    /// requests -- see `ProjectSkeleton::flattened` -- and this one is consulted first.
    pub(crate) flattened: RefCell<FxHashMap<Box<str>, Arc<MemberTable>>>,
    /// The same tables by the spelling a caller asked with, so the common ask -- the spelling the
    /// index already uses -- finds its table without folding the name into a new string first.
    pub(crate) flattened_as_written: RefCell<FxHashMap<Box<str>, Arc<MemberTable>>>,
    /// How many body-read return chains are being followed right now.
    ///
    /// A wrapper's type is the type of what it returns, and what it returns may be another wrapper:
    /// `$this->getDatabase()->select()` is two hops and either may be one of these. That is fine and
    /// is the point -- what is not is a cycle, which a checkout is entitled to contain, so the walk
    /// carries a depth rather than trusting the code it reads. See `returned_by_body`.
    pub(crate) following_returns: std::cell::Cell<u8>,
    /// Which stored declaration a class name belongs to, memoized for the same reasons and scoped
    /// the same way.
    ///
    /// It exists because finding one is a scan of every class the file declares, and the questions
    /// that need it -- what a class supplied for a supertype's templates, what its own templates are
    /// -- are asked for every member whose type is a name and for every `new`.
    pub(crate) declarations: RefCell<FxHashMap<Box<str>, Option<&'a phrost_index::ClassSkeleton>>>,
    /// The classes each trait asked about is composed into -- see
    /// [`Self::trait_hosts`](crate::IndexView::trait_hosts) -- memoized for the same reasons.
    pub(crate) trait_hosts: RefCell<FxHashMap<Box<str>, Arc<[phrost_model::Name]>>>,
    /// The table each model asked about has its rows in -- see
    /// [`Self::model_table`](crate::IndexView::model_table). A hierarchy climb, asked for every read
    /// of a model's attribute, and the same answer every time within a request.
    pub(crate) model_tables: RefCell<FxHashMap<Box<str>, Option<Arc<str>>>>,
    /// Whether a "does this call ever return" question is already being answered.
    ///
    /// That question resolves the call's receiver, and resolving anything consults the guards
    /// standing before it -- one of which may be the very call being asked about. The recursion is
    /// real and reachable: `aborter()->fail();` inside a guard resolves `aborter()`, which walks
    /// back to the same statement. A second ask gives up, which costs a guard that guards itself
    /// and nothing else.
    pub(crate) answering_never: std::cell::Cell<bool>,
    /// What each declaration of the open file throws beyond what its docblock says, by
    /// `owner::name`, memoized for as long as this view lives.
    ///
    /// A caller's throws read a same-file callee's through this, and a file of two hundred methods
    /// asks it for every call in every one of them. An entry is written empty before it is worked
    /// out, so a method that reaches itself reads as throwing what it throws and nothing more.
    pub(crate) undocumented_in_file: RefCell<FxHashMap<Box<str>, Vec<phrost_model::Name>>>,
    /// The open file's documented declarations, read once for the lookups above.
    pub(crate) documented: std::cell::OnceCell<Vec<crate::DocumentedDeclaration>>,
    /// What a value path resolved to, memoized for as long as this view lives.
    ///
    /// Scoped to one request for the same reason the member tables are: resolution is pure, so
    /// caching it is safe at any lifetime, and the *invalidation* is not.
    ///
    /// It exists because two whole-file projections ask the same questions. Every member access
    /// resolves its receiver, and every call resolves the receiver of its callee -- the same
    /// expressions, from two walks. Adding the second walk moved diagnostics from 6.2 ms to 8.7 ms
    /// p95 on a real project before this; with it, the second walk asks and is answered.
    ///
    /// Keyed by the position as well as the path, because a path means different things at different
    /// points in a body: `$x` after a reassignment is not the `$x` before it.
    ///
    /// Each answer carries whether a framework's contract changed anything on the way to it, which
    /// is what lets [`Self::with_contracts_lifted`] keep the answers no contract touched: for those
    /// the two worlds compute the same thing step for step.
    pub(crate) resolved: RefCell<ResolvedPaths>,
    /// What each variable of a template was rendered with, for as long as this view lives -- one
    /// template writes the same `$post` dozens of times, and each answer is a walk of the files
    /// rendering it. See `resolve::rendered`.
    pub(crate) template_variables: RefCell<FxHashMap<Box<str>, Option<Held>>>,
    /// The class whose instance renders this template, once asked. See `resolve::rendering`.
    pub(crate) template_class: std::cell::OnceCell<Option<phrost_model::Name>>,
    /// The classes a call configuring this file names for its bound closures, resolved, once
    /// asked. See `closure_this`.
    pub(crate) configured_this: std::cell::OnceCell<Vec<phrost_model::Name>>,
    /// Whether a contract has changed a type since this was last reset -- see `resolve_path`.
    pub(crate) contract_applied: std::cell::Cell<bool>,
    /// The block closures whose return type is being inferred, innermost last.
    ///
    /// A closure that names itself -- `use (&$fact)` and a recursive call -- asks what it returns
    /// while that is already being answered. The follow budget bounds the recursion but only after
    /// several re-entries, and each level multiplies by however many `return` statements the body
    /// has. Refusing re-entry outright makes the inner ask answer nothing, which is the honest
    /// answer for a type defined in terms of itself, and costs nothing anywhere else: a closure
    /// written inside another closure is a different [`phrost_hir::ExprId`].
    pub(crate) inferring_closures: RefCell<Vec<phrost_hir::ExprId>>,
    /// Every member access in the current file, memoized for as long as this view lives.
    ///
    /// Scoped to one request for the reason the others are. It exists because a diagnostics pass is
    /// several checks over one file and four of them ask this same question -- is the member there,
    /// may the receiver be null, is the receiver an object at all, is the member visible from here --
    /// and each ask used to re-walk every member site in the file and rebuild the vector.
    ///
    /// Shared rather than cloned: a large real file has a few thousand accesses, and three extra
    /// copies of that is the cost this was added to remove.
    pub(crate) accesses: RefCell<Option<Arc<[crate::references::MemberAccess]>>>,
    /// Every call in the current file, memoized for as long as this view lives.
    ///
    /// The argument checks are six walks of the same calls -- too many, too few, the wrong type, a
    /// sentinel, a by-reference write, a `new` that cannot be -- and each used to walk the whole tree
    /// for them again. Contract-free: which calls a file writes is syntax.
    pub(crate) calls: RefCell<Option<Arc<[phrost_body::CallSite]>>>,
    /// What each of those calls' callee takes, by the call's range and its callee's -- see
    /// `parameters_and_templates_of`. Not carried into the contracts-lifted twin: the callee is
    /// found through the receiver, and the receiver's type is what a contract can change.
    pub(crate) call_parameters: RefCell<CallParameters>,
    /// Which statement calls in the current file pass which path -- see
    /// `phrost_body::PassedPaths`. Syntax, so the contracts-lifted twin keeps it.
    pub(crate) passed_paths: RefCell<Option<Arc<phrost_body::PassedPaths>>>,
    /// Whether a statement's callee says anything about a path it was handed, for each statement
    /// asked so far -- see `IndexView::passed_to_calls`. What a callee says is a contract and is
    /// found through a receiver, so not carried into the contracts-lifted twin.
    pub(crate) speaking_statements: RefCell<SpeakingStatements>,
    /// What each statement and guard proved about a value, for the narrowing walk -- see
    /// `phrost_body::ProvedMemo`. Not carried into the contracts-lifted twin: its "never" resolves
    /// through a different world.
    pub(crate) proved: phrost_body::ProvedMemo,
    /// What each method call statement reaches, by where it is written, how far down a chain it is,
    /// and the budget it was asked with -- see `declaration_called`. Found through the receiver, so
    /// not carried into the contracts-lifted twin.
    pub(crate) declarations_called:
        RefCell<FxHashMap<(TextSize, usize, u8), Option<crate::locals::calls::CalledDeclaration>>>,
    /// Every property name the current file assigns to, memoized for as long as this view lives.
    ///
    /// One answer for the whole file, and asked once per member access: it is what rules the write
    /// question out for a slot nothing writes, and the precise answer to that question is a walk of
    /// the enclosing body -- see `phrost_body::assigned_properties`.
    pub(crate) assigned: RefCell<Option<Arc<[phrost_model::Name]>>>,
    /// Every existence test the current file writes, memoized for as long as this view lives.
    ///
    /// Asked whenever a magic accessor is about to answer for a name, which on a class with
    /// `__call` is every undeclared call -- and the walk that finds them visits every expression.
    pub(crate) existence: RefCell<Option<Arc<phrost_body::ExistenceTests>>>,
}

/// How many assignments a local's type is chased through before giving up.
///
/// Generous for real code and finite for code being typed: an editor sees `$a = $b; $b = $a;` on
/// the way to something valid, and a server that hangs on it is worse than one that gives up.
///
/// Twelve rather than eight, and the difference is measured rather than guessed. Every narrowing
/// this project adds spends a hop, so a chain that fitted yesterday can stop fitting today -- and
/// the failure is silent, because a receiver that runs out of budget reads exactly like one whose
/// type is genuinely unknown. Eight was costing four member accesses on the reference checkout.
/// Raising it costs nothing any budget can see: the limit only bites on chains deep enough to be
/// rare, so no p95 moves, and the worst case is a fixed multiple of a bounded walk rather than
/// anything that grows with the project.
///
/// Sixteen rather than twelve, for the same reason twelve replaced eight. A conditional return that
/// stood for its branches took one more hop on a Laravel loop and its local went untyped; measured
/// at sixteen, WordPress typed 101 more assignments and 131 more accesses, and an answer cut off
/// part-way is not always a silence -- it can be a *wider* type, which WordPress's `class-wp.php`
/// read as `array|string` and reported. `check --jobs 1` and the perf sweep did not move.
///
/// Twenty-four rather than sixteen, the third time for the same reason. `'' === $s` became a guard
/// when `non-empty-string` became a type, so a function testing its string for emptiness on the way
/// to a `str_replace` spends hops it did not before -- and Symfony's `AnsiUtils::visibleWidth`, three
/// such guards and four conditional writes deep, ran out exactly there: its `$subject is string`
/// answered both ways and the `string|string[]` was reported five times. Twenty was measured and
/// was not enough.
pub(crate) const FOLLOW_LIMIT: u8 = 24;

impl<'a> IndexView<'a> {
    /// A view for cross-file questions only.
    pub fn new(skeleton: &'a ProjectSkeleton, open: &'a OpenSet) -> Self {
        Self {
            skeleton,
            open,
            current: None,
            contracts: Contracts::Applied,
            flattened: RefCell::default(),
            flattened_as_written: RefCell::default(),
            following_returns: std::cell::Cell::new(0),
            declarations: RefCell::default(),
            model_tables: RefCell::default(),
            trait_hosts: RefCell::default(),
            resolved: RefCell::default(),
            contract_applied: std::cell::Cell::new(false),
            inferring_closures: RefCell::default(),
            accesses: RefCell::default(),
            calls: RefCell::default(),
            call_parameters: RefCell::default(),
            passed_paths: RefCell::default(),
            speaking_statements: RefCell::default(),
            proved: phrost_body::ProvedMemo::default(),
            declarations_called: RefCell::default(),
            assigned: RefCell::default(),
            existence: RefCell::default(),
            answering_never: std::cell::Cell::new(false),
            undocumented_in_file: RefCell::default(),
            documented: std::cell::OnceCell::new(),
            template_variables: RefCell::default(),
            template_class: std::cell::OnceCell::new(),
            configured_this: std::cell::OnceCell::new(),
        }
    }

    /// A view for a request about one file.
    pub fn in_file(skeleton: &'a ProjectSkeleton, open: &'a OpenSet, file: FileId) -> Self {
        Self {
            skeleton,
            open,
            current: Some(file),
            contracts: Contracts::Applied,
            flattened: RefCell::default(),
            flattened_as_written: RefCell::default(),
            following_returns: std::cell::Cell::new(0),
            declarations: RefCell::default(),
            model_tables: RefCell::default(),
            trait_hosts: RefCell::default(),
            resolved: RefCell::default(),
            contract_applied: std::cell::Cell::new(false),
            inferring_closures: RefCell::default(),
            accesses: RefCell::default(),
            calls: RefCell::default(),
            call_parameters: RefCell::default(),
            passed_paths: RefCell::default(),
            speaking_statements: RefCell::default(),
            proved: phrost_body::ProvedMemo::default(),
            declarations_called: RefCell::default(),
            assigned: RefCell::default(),
            existence: RefCell::default(),
            answering_never: std::cell::Cell::new(false),
            undocumented_in_file: RefCell::default(),
            documented: std::cell::OnceCell::new(),
            template_variables: RefCell::default(),
            template_class: std::cell::OnceCell::new(),
            configured_this: std::cell::OnceCell::new(),
        }
    }

    /// The same file, read as if no framework had contracted a sentinel away.
    ///
    /// For one question and no other: which of the checks that stayed quiet did so *because* of a
    /// contract. Running them again against this twin answers it without a second rule about
    /// nullability living anywhere -- the shape §8 calls two constructors of one rule, and the one
    /// this is written to avoid.
    ///
    /// A whole view rather than a flag on the call, because resolution is memoized: a path resolved
    /// with contracts lifted is a different answer from the same path resolved with them applied,
    /// and sharing one cache between the two would hand whichever asked second the other's answer.
    ///
    /// What a contract cannot change is carried over instead: a class's member table, its stored
    /// declaration, the two whole-file walks of what the file writes, and every resolved path whose
    /// answer no contract changed. The last is the one that pays -- re-resolving the whole file was
    /// seven of the nine milliseconds `assumed-non-null` cost on the reference project's largest
    /// file, for a single call a contract had spoken about.
    pub(crate) fn with_contracts_lifted(&self) -> IndexView<'a> {
        IndexView {
            skeleton: self.skeleton,
            open: self.open,
            current: self.current,
            contracts: Contracts::Lifted,
            flattened: RefCell::new(self.flattened.borrow().clone()),
            flattened_as_written: RefCell::new(self.flattened_as_written.borrow().clone()),
            following_returns: std::cell::Cell::new(0),
            declarations: RefCell::new(self.declarations.borrow().clone()),
            model_tables: RefCell::new(self.model_tables.borrow().clone()),
            trait_hosts: RefCell::new(self.trait_hosts.borrow().clone()),
            // Only what no contract touched, which is the same answer in both worlds.
            resolved: RefCell::new(
                self.resolved
                    .borrow()
                    .iter()
                    .map(|(key, answers)| {
                        let kept = answers
                            .iter()
                            .filter(|(_, (_, touched))| !touched)
                            .cloned()
                            .collect::<Vec<_>>();
                        (*key, kept)
                    })
                    .filter(|(_, kept)| !kept.is_empty())
                    .collect(),
            ),
            contract_applied: std::cell::Cell::new(false),
            inferring_closures: RefCell::default(),
            accesses: RefCell::default(),
            calls: RefCell::new(self.calls.borrow().clone()),
            call_parameters: RefCell::default(),
            passed_paths: RefCell::new(self.passed_paths.borrow().clone()),
            speaking_statements: RefCell::default(),
            proved: phrost_body::ProvedMemo::default(),
            declarations_called: RefCell::default(),
            assigned: RefCell::new(self.assigned.borrow().clone()),
            existence: RefCell::new(self.existence.borrow().clone()),
            answering_never: std::cell::Cell::new(false),
            undocumented_in_file: RefCell::default(),
            documented: std::cell::OnceCell::new(),
            template_variables: RefCell::default(),
            template_class: std::cell::OnceCell::new(),
            configured_this: std::cell::OnceCell::new(),
        }
    }

    pub fn skeleton(&self) -> &'a ProjectSkeleton {
        self.skeleton
    }

    /// The buffer the request is about, if it is open.
    ///
    /// Only the *current* file is reachable. Other open buffers are deliberately out of reach: a
    /// cross-file answer that consulted them would start depending on which files the user happens
    /// to have open, which is the behaviour this design exists to rule out.
    /// The file the request is about, whether or not it is open.
    pub fn current_file_id(&self) -> Option<FileId> {
        self.current
    }

    pub(crate) fn current_file(&self) -> Option<FileId> {
        self.current
    }

    /// Whether the file this request is about was written under `declare(strict_types=1)`.
    ///
    /// Deliberately not askable about any other file. PHP applies the directive where a call is
    /// *written*, never where the callee was declared, so a check that reached for the callee's
    /// file would answer the opposite question on every cross-file call. Leaving the file out of
    /// the signature is what makes that mistake unavailable rather than merely discouraged.
    pub fn strict_types(&self) -> bool {
        self.current
            .and_then(|file| self.skeleton.file(file))
            .is_some_and(|file| file.strict_types())
    }

    pub(crate) fn current(&self) -> Option<&'a OpenFile> {
        self.open.get(self.current?)
    }

    /// The house style this project's code is written in, by the name a framework gave it.
    ///
    /// A name and not a decision: what "drupal" means to a docblock is the feature layer's to know,
    /// and this crate has no opinion about how a comment is laid out.
    pub fn house_style(&self) -> Option<&'a str> {
        self.skeleton.house_style()
    }

    pub fn file_path(&self, file: FileId) -> Option<&'a str> {
        self.skeleton.display_path_of(file)
    }

    /// The docblock attached to a declaration, as its author wrote it.
    ///
    /// Read on demand rather than retained: keeping every docblock of every vendor class resident is
    /// tens of megabytes on a real project, and reading one file to answer one hover is well inside
    /// the budget.
    ///
    /// An open buffer is preferred over disk for any file, not only the current one — the one place
    /// this view looks past the current buffer, and a correctness requirement rather than a
    /// convenience. When a file is open, the index ingested *that text*, so its ranges are offsets
    /// into the buffer; slicing the disk copy instead would cut at the wrong places.
    pub fn documentation(&self, location: &SymbolLocation) -> Option<String> {
        let range = location.docblock_range?;

        if let Some(open) = self.open.get(location.file) {
            return open
                .text()
                .get(usize::from(range.start())..usize::from(range.end()))
                .map(str::to_owned);
        }

        self.skeleton.text_slice(location.file, range)
    }

    /// The source between two offsets of a declaration's own file.
    ///
    /// The same read as [`Self::documentation`], for the same reason and with the same rule: the
    /// index keeps ranges, not text, so anything that wants the characters a declaration was written
    /// with asks for them once, when a person is looking at that one declaration. An open buffer
    /// answers from what the editor holds, so this cannot disagree with the screen.
    pub fn source_between(&self, file: FileId, range: TextRange) -> Option<String> {
        if let Some(open) = self.open.get(file) {
            return open
                .text()
                .get(usize::from(range.start())..usize::from(range.end()))
                .map(str::to_owned);
        }

        self.skeleton.text_slice(file, range)
    }

    /// The text of the buffer the request is about.
    ///
    /// Completion needs it, and cannot get by without it: at the moment someone asks, the code
    /// usually does not parse — `$this->` on its own is not an expression — so the tree has nothing
    /// to say about where the cursor is. The characters before it do.
    pub fn current_text(&self) -> Option<&'a str> {
        Some(self.current()?.text())
    }
}

pub(crate) fn hir_member_kind(kind: phrost_hir::MemberKind) -> MemberKind {
    match kind {
        phrost_hir::MemberKind::Method => MemberKind::Method,
        phrost_hir::MemberKind::Property => MemberKind::Property,
        phrost_hir::MemberKind::ClassConstant => MemberKind::ClassConstant,
        phrost_hir::MemberKind::EnumCase => MemberKind::EnumCase,
    }
}

pub(crate) fn member_symbol_kind(kind: MemberKind) -> SymbolKind {
    match kind {
        MemberKind::Method => SymbolKind::Method,
        MemberKind::Property => SymbolKind::Property,
        MemberKind::ClassConstant => SymbolKind::ClassConstant,
        MemberKind::EnumCase => SymbolKind::EnumCase,
    }
}
