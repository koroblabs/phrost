use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use phrost_defmap::{DeclarationSource, FileScope};
use phrost_hir::{FileHir, ImportKind};
use phrost_lower::KeyedAttributes;
use phrost_model::{FileId, Interner, Name, NameKey, SourceOrigin, SymbolKind};
use phrost_model::{TextRange, short_name};
use phrost_sema::{
    ClassFacts, ClassKind, ClassRef, ClassSource, FlattenMemo, MemberKind, MemberModifiers,
    MemberRow, MemberTable, TraitAdaptation, TraitAdaptationKind,
};
use phrost_ty::TypeTable;
use phrost_vfs::{FileFingerprint, FileStamp, SourceText};
use rustc_hash::FxHashMap;

use crate::{
    dispatch::DispatchRow,
    function::FunctionRow,
    owners::{CatalogKey, Declarer, OwnerCatalog},
    returns::ReturnContracts,
    state::IndexState,
    types::TypeLowerer,
};

/// The project-wide skeleton: what every file declares, and how those declarations relate.
///
/// Lean on purpose. It holds declared names, import scopes, inheritance edges, and the ranges
/// needed to navigate to a declaration — and no source text, no parse trees, and no expression
/// bodies. That is what keeps resident memory a function of how much code the project has rather
/// than how large its `vendor/` directory is.
///
/// It is also the reason there is no fallback path anywhere above: a definition range is kept for
/// every declaration in every file, including vendor, so "the index has no answer" always means
/// the symbol genuinely is not declared, never that the answer was not retained.
/// How many flattened member tables the skeleton keeps between requests -- see
/// [`ProjectSkeleton::remember_flattened`].
const FLATTENED_LIMIT: usize = 256;

/// What one file said that its rows do not hold: what only its bodies and kept defaults could
/// state, and which travels through the codec because a warm start never lowers anything again.
///
/// One value rather than one argument per list, because every list has to take the same three
/// routes -- lowered, cached, adopted -- and an argument list that grows by one at each new kind is
/// three call sites to remember.
#[derive(Default)]
pub(crate) struct Said {
    pub(crate) dispatches: Box<[DispatchRow]>,
    pub(crate) tables: Box<[crate::tables::FoundTable]>,
    pub(crate) stated: Box<[crate::StatedTable]>,
    pub(crate) renders: Box<[crate::RenderSite]>,
    pub(crate) configured_this: Box<[crate::ConfiguredThis]>,
    pub(crate) schema: crate::schema::FileSchema,
    pub(crate) class_defaults: Box<[crate::class_defaults::ClassDefault]>,
    pub(crate) constructions: Box<[crate::constructions::ConstructionSite]>,
    pub(crate) macros: Box<[crate::macros::MacroSite]>,
}

/// What turns a template into PHP of the same length, and how many constructs it could not write
/// in place.
pub type TemplateToPhp = fn(&str) -> (String, usize);

/// A template language read as the PHP it holds. The framework seam's `TemplateDialect`, restated
/// below it for the reason the contracts are.
#[derive(Clone, Debug)]
pub struct TemplateDialect {
    pub suffix: Box<str>,
    pub to_php: TemplateToPhp,
    /// Where templates a call names live, from the project root.
    pub directory: Box<str>,
    /// What a template's name writes between directories.
    pub separator: Box<str>,
    /// Variables every template has, and the class each holds.
    pub shared: SharedVariables,
    /// The prefix the template a component tag names has, and the variables every such template
    /// has besides.
    pub components: Box<[(Box<str>, SharedVariables)]>,
    /// The functions `to_php` writes calls to that no file declares: a component tag's, an
    /// include's.
    pub calls: Box<[Box<str>]>,
    pub problems: TemplateProblems,
}

/// What in a template's structure its PHP cannot carry, and where, in bytes of the template.
pub(crate) type TemplateProblems = fn(&str) -> Vec<(std::ops::Range<usize>, String)>;

/// Variables a template has whatever renders it, and the class each holds.
pub(crate) type SharedVariables = Box<[(Box<str>, Box<str>)]>;

/// The dialect whose file-name ending `path` has.
pub(crate) fn template_dialect(dialects: &[TemplateDialect], path: &Path) -> Option<TemplateToPhp> {
    if dialects.is_empty() {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    dialects
        .iter()
        .find(|dialect| name.len() > dialect.suffix.len() && name.ends_with(&*dialect.suffix))
        .map(|dialect| dialect.to_php)
}

#[derive(Debug)]
pub struct ProjectSkeleton {
    /// Project root, for turning a stored path into the short one a person reads.
    ///
    /// Kept once rather than storing a relative path per file: one path per file is enough, and it
    /// has to be the absolute one because reading a file needs it.
    root: Option<PathBuf>,
    /// Name storage for the whole project, so one name is one allocation.
    interner: Interner,
    /// Type storage for the whole project, for the same reason and with the same effect: a member
    /// carries a `TypeId` into this rather than its own copy of `?string`.
    types: TypeTable,
    /// What the engine binds in every scope, and what each one holds.
    ///
    /// Ids into [`Self::types`], so the two travel together: a skeleton that adopts another's
    /// table has to take its map with it or every id means something else. See
    /// [`crate::superglobals`] for why this is a table here rather than a stub file.
    superglobals: FxHashMap<Name, phrost_ty::TypeId>,
    /// The `array{...}` each property table describes, and the list of one per row, interned.
    ///
    /// Beside [`Self::superglobals`] and rebuilt for exactly the reason [`Self::adopt`] states
    /// about them: an id is an index into the table it was interned in, and adopting replaces that
    /// table. The tables themselves live in `returns`, which adopting deliberately keeps -- so
    /// these are the derived half and are made again whenever the type table is.
    property_shapes: FxHashMap<(Box<str>, Box<str>), PropertyShape>,
    files: FxHashMap<FileId, FileSkeleton>,
    owners: OwnerCatalog,
    state: IndexState,
    /// What a framework knows about a declaration that the declaration gets wrong.
    ///
    /// Installed from above and never derived from a file, which is why [`Self::adopt`] keeps it
    /// across a reload while replacing everything that is.
    returns: ReturnContracts,
    /// Where the autoloader maps a namespace to. See [`Self::file_scope_is_private`].
    source_roots: Vec<std::path::PathBuf>,
    /// The same directories with the namespace each stands for -- see
    /// [`Self::permitted_namespaces`].
    namespace_rules: Vec<(Box<str>, std::path::PathBuf)>,
    /// Namespace segments whose classes a framework finds by scanning -- see
    /// `FrameworkProvider::discovered_namespaces`. Set with the other contributions and never
    /// cached, like the rules beside it.
    /// Whether composer calls this package a `project` rather than something with consumers.
    ///
    /// Set with the other contributions and never cached. Asked by the one check whose answer
    /// inverts on it: what nothing here calls is dead code in an application and the public API
    /// in a library.
    is_an_application: bool,
    discovered_namespaces: Vec<Box<str>>,
    /// Supertypes whose subclasses a framework instantiates -- see `discovered_supertypes`.
    discovered_supertypes: Vec<Box<str>>,
    /// Supertypes whose subclasses a framework *runs*, catching what they throw -- see
    /// `runs_and_catches`. Apart from the list above because the two say different things: one is
    /// about a class being built and this is about its exceptions having no caller to be told to.
    run_supertypes: Vec<Box<str>>,
    /// Prefixes that make a string literal name a method of the class it is written in -- see
    /// `own_method_prefixes`. Drupal's `'::districtCallback'` is the one that exists.
    own_method_prefixes: Vec<Box<str>>,
    /// Calls on a test double whose string argument names a method of what it stands in for -- see
    /// `stand_in_methods`.
    stand_in_methods: Vec<StandInMethod>,
    /// Calls whose string argument names a key a sibling method writes into an array -- see
    /// `built_keys`.
    built_keys: Vec<BuiltKey>,
    /// Supertypes whose subclasses answer names no declaration holds -- see `dynamic_members`.
    ///
    /// A third list on the same climb and apart from the two above for the same reason: what a
    /// framework says about one hierarchy can be three different things.
    dynamic_supertypes: Vec<Box<str>>,
    /// Where the project's configuration names a class -- see [`ConfigSite`](crate::ConfigSite).
    ///
    /// Beside the namespace rules and not in the file table, and re-derived at every start for the
    /// same reason they are: it is cheap, and a stored copy is a thing `adopt` has to remember.
    config_sites: Vec<crate::ConfigSite>,
    /// Every word the project's own templates write, with the getter spellings of each.
    ///
    /// Installed rather than derived, like [`Self::config_sites`] beside it: both are read from
    /// files the index never holds. See `Workspace::install_template_words` for what a word buys
    /// and why it is a word rather than a call.
    template_words: std::collections::HashSet<Box<str>>,
    /// Where the project's own templates render another by a name written in them -- an `include`,
    /// an `embed`, an `extends` -- as the including template and the name. Installed beside
    /// [`Self::template_words`] from the same read, for the same reason it is not cached.
    twig_includes: Vec<(std::sync::Arc<Path>, Box<str>)>,
    /// The house style this project's code is written in, by the name a framework gave it.
    ///
    /// Configuration, like the contracts beside it: it describes the project rather than any file,
    /// so it survives a re-index and is not carried across from a loaded one.
    style: Option<Box<str>>,
    /// Global names a framework's loader `class_alias`es at start, folded, to the class each names.
    ///
    /// Configuration, so kept across `adopt` -- and text rather than interned names for the reason
    /// the contracts are: adopting replaces the interner. See [`Self::class_declarers`].
    class_aliases: FxHashMap<Box<str>, Box<str>>,
    /// Template languages read as the PHP they hold: a file-name ending, and what turns the
    /// template into PHP of the same length. Configuration, so kept across `adopt`. See
    /// [`Self::as_php`].
    template_dialects: Vec<TemplateDialect>,
    /// Which attributes lowering keeps a positional string literal for.
    ///
    /// Configuration for the same reason as the contracts above -- it is derived from which
    /// frameworks are present, not from any file -- and it has to reach lowering, because a
    /// declaration a file did not have lowered is a key the catalog never hears about.
    pub(crate) keyed_attributes: KeyedAttributes,
    /// Flattened member tables kept from one request to the next.
    ///
    /// `IndexView` memoizes them for one request, which is where a single pass asks about `$this`
    /// hundreds of times. Across requests it is the same few classes again -- every Drupal form
    /// flattens `FormBase` and everything above it, and a diagnostics publish on each keystroke
    /// built the same tables every time. Measured on `check` over 1 711 files, flattening was a
    /// fifth of the pass.
    ///
    /// Kept here rather than in the workspace because this is where a class changes: `install` and
    /// `retract` know which classes a file declared, and a table records the classes it was built
    /// from, so the two meet in one place and nothing has to remember to clear it. A `Mutex`
    /// because requests read the skeleton under a shared lock; it is taken for a lookup and never
    /// held across one.
    flattened: Mutex<FlattenedMemo>,
    /// What the project's own code was seen to *reach*, resolved — see [`Reached`].
    ///
    /// A `Mutex` for the same reason the memo above is one, and it is the same kind of thing:
    /// derived from an analysis somebody was doing anyway, written under a shared lock, and worth
    /// nothing to a cache on disk because the analysis of an old tree says nothing about a new
    /// one. Taken for one file's contribution or one lookup and never held across either.
    reached: Mutex<crate::Reached>,
    /// Every name some declaration in the project returns `never` from.
    ///
    /// A filter in front of a resolution, and it is *exact* rather than a guess: a call can only
    /// leave the function for good if the thing it names is declared to, so a name nothing declares
    /// that way needs no resolution to rule out. Deciding it the other way means reading the
    /// callee's declaration, and finding the callee means resolving its receiver -- a full chain
    /// resolution to learn one bit.
    ///
    /// What the list costs therefore scales with how much a project actually uses `never`, and that
    /// is a property of the project rather than of this design. A codebase built around
    /// `Assert::fail()` and `never`-returning guards puts more names here and resolves more often,
    /// and is still strictly better off than resolving every call. One data point for the shape of
    /// it, not for the rule: a production Drupal checkout declares four in total, two of which
    /// (`die`, `exit`) are syntax and never reach here.
    ///
    /// A `Vec` scanned rather than a set, for the reason [`ReturnContracts::from_argument`] records:
    /// PHP compares these names without regard to case, so a hashed lookup has to fold the name
    /// first, and folding allocates on a path every trailing statement of every block takes. If a
    /// project ever puts enough names here for the scan to show up, that is the point to measure a
    /// folded set against it -- not before.
    ///
    /// Only ever added to. A file that stops declaring one leaves its name behind, which costs one
    /// pointless resolution and can never produce a wrong answer -- the resolution is still the
    /// thing that decides. A superset is the safe direction here, and withdrawing keys is only worth
    /// its complexity where a stale one would be *believed*.
    never_returning: Vec<Name>,
    /// Short names of every class the project binds a closure to -- see `FileSkeleton::bound_scopes`.
    bound_scopes: Vec<Name>,
    /// The names of properties that carry a `set` hook somewhere in the project.
    ///
    /// The suppressor in front of the read-only-property rule. A property whose hooks are a `get`
    /// alone refuses every write -- PHP says `Property C::$name is read-only` -- but a *subclass*
    /// may override the property and add a `set`, and a write through the parent's type then runs.
    /// Verified against PHP 8.5.10 rather than argued.
    ///
    /// So the rule is asked by name, and accumulating rather than withdrawn with the file that
    /// wrote it: both make it suppress more than it must, which is the direction a check about the
    /// *absence* of a declaration has to be wrong in. The same shape and the same justification as
    /// [`Self::bound_scopes`].
    ///
    /// Narrowed to declarations that could actually *be* the override, which is what keeps a common
    /// name from suppressing the rule everywhere: only a class-like that extends something can be a
    /// subclass of the owner, and only a trait can put a `set` into one from the side. A class with
    /// no parent declaring a set-hooked `$name` says nothing about anybody else's `$name`.
    set_hooked: Vec<Name>,
    /// The folded short name of every class declared `#[Attribute]`, sorted.
    ///
    /// The prefilter in front of attribute completion, and the same economics as
    /// [`Self::never_returning`]: `#[` can be followed by any class-like the catalog holds, and the
    /// ones that could *carry* an attribute are 441 of 30 916 on a real checkout. Ranking them
    /// first by walking the catalog and asking each class means a class lookup per name on every
    /// keystroke; naming them outright is a list the size of how many a project declares.
    ///
    /// Sorted rather than in claim order, because files are indexed in parallel and a completion
    /// list that reshuffles between two identical sessions is worse than one that is merely long.
    ///
    /// Only ever added to, for the reason the two lists above record. A class that stops being an
    /// attribute leaves its name here and is offered in front of the others once more; the
    /// projection still asks its skeleton before believing it, so a stale name costs one lookup and
    /// cannot produce a wrong row.
    attributes: Vec<NameKey>,
    /// The names of callables carrying an `@phpstan-assert-if-true`/`-if-false` about an
    /// *argument*, for the same reason and with the same economics as [`Self::never_returning`].
    ///
    /// The question this answers is asked of every `if (f($x))` in a project: is `f` worth building
    /// a narrowing root for. Answered by resolution it is a declaration lookup per guard, and the
    /// answer is no for nearly all of them -- so it is a name scan, and what it costs scales with
    /// how much a project documents its guards rather than with how many it writes. A production
    /// WordPress checkout puts two names here.
    ///
    /// Argument assertions only, and only the suffixed forms. A tag about the *receiver* is read
    /// through [`crate::PathRoot::Asserted`], which names the method and needs no gate -- the
    /// receiver is the narrowed value, so there is nothing to guess. An unsuffixed `@phpstan-assert`
    /// is applied at the call site itself and never through a guard.
    ///
    /// Only ever added to, for the reason the list above records: a superset costs one pointless
    /// lookup and can never produce a wrong answer, because the declaration is still what decides.
    asserting_arguments: Vec<Name>,
    /// The names of methods that say what their receiver holds once they return --
    /// `@phpstan-self-out`, `@phpstan-this-out` and their spellings -- for the same reason and with
    /// the same economics as [`Self::never_returning`].
    ///
    /// The question this answers is asked of every call statement on every value a body resolves:
    /// did `$x->m(...)` change what `$x` is. Answered by resolution it is a receiver and a
    /// declaration looked up per statement, and the answer is no for nearly all of them -- Laravel
    /// writes the tag on seven methods in its whole tree -- so the call-statement index only
    /// records a receiver whose method name is here, and what it costs scales with how many such
    /// methods a project declares rather than with how many calls it writes.
    ///
    /// Only ever added to, for the reason the lists above record: a superset costs one pointless
    /// lookup and can never produce a wrong answer, because the declaration is still what decides.
    receiver_out: Vec<Name>,
    /// The extension points each file's own calls declare.
    ///
    /// A table keyed by file rather than a field on the file's row, and that is a memory decision
    /// with a number behind it: 30 909 of 30 916 files on a real project have nothing to put in one,
    /// and a slice header per file is a megabyte of empty headers to hold seven rows. Keyed, so a
    /// re-ingest still withdraws exactly what the file claimed rather than scanning -- the mistake
    /// §8 records about `remove_file`.
    ///
    /// Derived from a file's body, so it travels with the files: [`Self::adopt`] takes it, and
    /// [`Self::install`] and [`Self::retract`] are the only writers.
    dispatches: FxHashMap<FileId, Box<[DispatchRow]>>,
    /// The property tables each file's own declarations state, keyed the way `dispatches` is and
    /// for the same reason: they are what a *file* said, so they travel with the files and are
    /// withdrawn with them.
    declared_tables: FxHashMap<FileId, Box<[crate::tables::FoundTable]>>,
    /// The properties each file's classes state in an array they write -- see
    /// [`StatedTable`](crate::StatedTable). Keyed by file for the reason `declared_tables` is, and
    /// beside rather than on the file row so that only a framework's models pay for it.
    stated: FxHashMap<FileId, Box<[crate::StatedTable]>>,
    /// The calls each file of the project's own code makes that render a template by name -- see
    /// [`crate::RenderSite`]. Keyed by file for the reason `stated` is.
    pub(crate) renders: FxHashMap<FileId, Box<[crate::RenderSite]>>,
    /// The calls each file makes naming the class a callee's bound closures run as -- see
    /// [`crate::ConfiguredThis`]. Keyed by file for the reason `renders` is.
    pub(crate) configured_this: FxHashMap<FileId, Box<[crate::ConfiguredThis]>>,
    /// What each file says about the database schema -- see [`crate::schema`]. Keyed by file for
    /// the reason `stated` is.
    pub(crate) schemas: FxHashMap<FileId, crate::schema::FileSchema>,
    /// The tables those add up to, composed on the first question after any of them changed: every
    /// write to `schemas` replaces this with an empty one.
    schema: std::sync::OnceLock<Arc<crate::schema::Schema>>,
    /// The classes each file's property defaults name -- see [`crate::class_defaults`]. Keyed by
    /// file for the reason `stated` is.
    class_defaults: FxHashMap<FileId, Box<[crate::class_defaults::ClassDefault]>>,
    /// The entries each file's kept constructions declare -- see [`crate::constructions`]. Keyed
    /// by file for the reason `stated` is, and holding only the files that declare one.
    constructions: FxHashMap<FileId, Box<[crate::constructions::ConstructionSite]>>,
    /// The methods each file's bodies give classes at runtime -- see [`crate::macros`]. Keyed by
    /// file for the reason `stated` is, and holding only the files that register one, so asking
    /// every one of them is asking a handful.
    macros: FxHashMap<FileId, Box<[crate::macros::MacroSite]>>,
    /// The files declaring a class that composes each trait, keyed by the trait's folded name --
    /// see [`Self::files_using_trait`]. Sorted, so an install finds its place by search.
    ///
    /// What `$this` inside a trait is, is the classes it is composed into, and finding them by
    /// walking the class table cost 10 ms a hover on a Drupal checkout of 31 000 files, every
    /// request: the walk is one pass over every class the project reaches, for an answer that is a
    /// handful of them. Derived from the files, so it travels with them like `dispatches`.
    trait_users: FxHashMap<u64, Vec<FileId>>,
}

/// Where a project's resident bytes went.
///
/// One field per thing worth defending separately, because the rule this measures is about *shape*
/// rather than total: the skeleton and the open buffers may be large, and the catalog and the type
/// table must not scale with `vendor/`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SkeletonMemory {
    /// Every interned string, once. The one place a name's bytes are counted.
    pub names: usize,
    /// The project-wide type table.
    pub types: usize,
    /// One absolute path per file, which reading a file needs.
    pub paths: usize,
    /// Namespaces, imports, and declared symbols, per file.
    pub scopes: usize,
    /// Class rows and their supertype lists.
    pub classes: usize,
    /// Member rows, which are the bulk of it on any real project.
    pub members: usize,
    /// Free function rows.
    pub functions: usize,
    /// The map from a name to the files that declare it, and the sorted short-name index.
    pub catalog: usize,
    /// The file table itself.
    pub files: usize,
    /// The words each of the project's own files holds, which a reference sweep reads instead of
    /// reading the project. Zero for a session that has indexed no user code.
    pub words: usize,
}

impl SkeletonMemory {
    pub fn total(&self) -> usize {
        self.names
            + self.types
            + self.paths
            + self.scopes
            + self.classes
            + self.members
            + self.functions
            + self.catalog
            + self.files
            + self.words
    }

    /// The rows a report prints, largest first.
    pub fn rows(&self) -> Vec<(&'static str, usize)> {
        let mut rows = vec![
            ("members", self.members),
            ("names", self.names),
            ("catalog", self.catalog),
            ("scopes", self.scopes),
            ("classes", self.classes),
            ("types", self.types),
            ("paths", self.paths),
            ("functions", self.functions),
            ("file table", self.files),
            ("words", self.words),
        ];
        rows.sort_by_key(|(_, bytes)| std::cmp::Reverse(*bytes));
        rows
    }
}

/// Written out rather than derived, because one field is built *from* two others: the engine's
/// variable types are interned into this skeleton's own table, so a skeleton assembled any other
/// way would hold ids into a table that does not have them. Deriving it left every
/// `Workspace::default()` -- which is how the server and every test build one -- with a table the
/// engine had never been installed into, and `$_SERVER` untyped in exactly the places it was
/// being measured.
impl Default for ProjectSkeleton {
    fn default() -> Self {
        let mut interner = Interner::default();
        let mut types = TypeTable::default();
        let superglobals = crate::superglobals::install(&mut types, &mut interner);

        Self {
            root: None,
            interner,
            types,
            superglobals,
            property_shapes: FxHashMap::default(),
            files: FxHashMap::default(),
            owners: OwnerCatalog::default(),
            state: IndexState::default(),
            returns: ReturnContracts::default(),
            source_roots: Vec::new(),
            namespace_rules: Vec::new(),
            config_sites: Vec::new(),
            template_words: std::collections::HashSet::new(),
            twig_includes: Vec::new(),
            is_an_application: false,
            discovered_namespaces: Vec::new(),
            discovered_supertypes: Vec::new(),
            run_supertypes: Vec::new(),
            dynamic_supertypes: Vec::new(),
            own_method_prefixes: Vec::new(),
            stand_in_methods: Vec::new(),
            built_keys: Vec::new(),
            style: None,
            class_aliases: FxHashMap::default(),
            template_dialects: Vec::new(),
            keyed_attributes: KeyedAttributes::default(),
            flattened: Mutex::default(),
            reached: Mutex::default(),
            never_returning: Vec::new(),
            bound_scopes: Vec::new(),
            set_hooked: Vec::new(),
            attributes: Vec::new(),
            asserting_arguments: Vec::new(),
            receiver_out: Vec::new(),
            dispatches: FxHashMap::default(),
            declared_tables: FxHashMap::default(),
            stated: FxHashMap::default(),
            renders: FxHashMap::default(),
            configured_this: FxHashMap::default(),
            schemas: FxHashMap::default(),
            schema: std::sync::OnceLock::new(),
            class_defaults: FxHashMap::default(),
            constructions: FxHashMap::default(),
            macros: FxHashMap::default(),
            trait_users: FxHashMap::default(),
        }
    }
}

/// The two interned types a property table describes.
///
/// Both, because the two readings of one table are asked at different links of a chain and neither
/// can be derived from the other without the table open for writing: `->value` wants the property
/// and `getValue()` wants a list of the whole shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PropertyShape {
    /// `array{date: string, rate: float}` -- only for a table that says it is complete.
    pub one: phrost_ty::TypeId,
    /// `array<int, array{...}>`, which is what one row per delta is.
    pub rows: phrost_ty::TypeId,
}

impl ProjectSkeleton {
    /// Where this index's heap went, by part.
    ///
    /// Walks everything rather than keeping a running total: it is asked once, by a person, and a
    /// counter maintained on every ingest is a second source of truth that can disagree with the
    /// first.
    pub fn memory(&self) -> SkeletonMemory {
        use lspkit_memsize::MemSize;

        let mut found = SkeletonMemory {
            names: self.interner.heap_size(),
            types: self.types.heap_size(),
            // The dispatch table with it: it is catalog data, and on a real project it is a few
            // hundred bytes -- a row of its own would be a line of the report that never moves.
            catalog: self.owners.heap_size()
                + lspkit_memsize::table::<FileId, Box<[crate::RenderSite]>>(self.renders.len())
                + lspkit_memsize::table::<FileId, Box<[crate::ConfiguredThis]>>(
                    self.configured_this.len(),
                )
                + lspkit_memsize::table::<FileId, Box<[DispatchRow]>>(self.dispatches.len())
                + self
                    .dispatches
                    .values()
                    .map(|rows| lspkit_memsize::deep(rows))
                    .sum::<usize>()
                + lspkit_memsize::table::<FileId, crate::schema::FileSchema>(self.schemas.len())
                + self.schemas.values().map(MemSize::heap_size).sum::<usize>()
                + self
                    .class_defaults
                    .values()
                    .map(|kept| lspkit_memsize::deep(kept))
                    .sum::<usize>()
                + self
                    .constructions
                    .values()
                    .map(|kept| lspkit_memsize::deep(kept))
                    .sum::<usize>()
                + self
                    .macros
                    .values()
                    .map(|kept| lspkit_memsize::deep(kept))
                    .sum::<usize>()
                + lspkit_memsize::table::<u64, Vec<FileId>>(self.trait_users.len())
                + self
                    .trait_users
                    .values()
                    .map(|files| files.capacity() * std::mem::size_of::<FileId>())
                    .sum::<usize>(),
            files: lspkit_memsize::table::<FileId, FileSkeleton>(self.files.len()),
            ..SkeletonMemory::default()
        };

        for file in self.files.values() {
            // The path is an `Arc<Path>`, and the file table is the only holder, so its bytes belong
            // here.
            found.paths += file.path.as_os_str().len();
            found.scopes += file.scope.heap_size();
            found.words += file.words.as_ref().map_or(0, MemSize::heap_size);
            found.classes += lspkit_memsize::vec_flat(&file.classes);
            found.functions += lspkit_memsize::vec_flat(&file.functions);
            // Beside the functions: the other free declaration with a stored signature. A row is
            // flat but for the rare value built from other constants.
            found.functions += lspkit_memsize::deep(&file.constants);
            // With the members, because that is what a `@throws` is a fact about and because the
            // table is sparse enough that a row of its own would never move: 9 500 declarations of
            // a quarter of a million on a real project.
            found.members += lspkit_memsize::deep(&file.throws);
            found.members += lspkit_memsize::deep(&file.returned_paths);
            // With the catalog rather than in a row of its own: it is catalog data that happens to
            // be stored per file, and a row for what a real project measures at a few hundred bytes
            // would be a line of the report that never says anything.

            for class in &file.classes {
                found.classes += lspkit_memsize::vec_flat(&class.extends)
                    + lspkit_memsize::vec_flat(&class.implements)
                    + lspkit_memsize::vec_flat(&class.uses)
                    + lspkit_memsize::vec_flat(&class.adaptations)
                    + lspkit_memsize::vec_flat(&class.templates);
                found.members += lspkit_memsize::vec(&class.members);
            }
        }

        found
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// Fixes the project root, which is what display paths are relative to.
    pub fn set_root(&mut self, root: impl Into<PathBuf>) {
        self.root = Some(root.into());
    }

    /// The project root, for writing it to a cache.
    pub(crate) fn root(&self) -> Option<&PathBuf> {
        self.root.as_ref()
    }

    /// Every file, in a deterministic order, for writing them to a cache.
    pub(crate) fn files_in_path_order(&self) -> Vec<(FileId, &FileSkeleton)> {
        let mut files = self
            .files
            .iter()
            .map(|(&file, entry)| (file, entry))
            .collect::<Vec<_>>();
        files.sort_by_key(|(_, entry)| entry.path());
        files
    }

    /// Rebuilds a skeleton from what a cache held.
    ///
    /// The catalog is not restored, it is *re-derived*: every file claims its names through the same
    /// method a fresh ingest uses. Storing it would be storing the same fact twice, and that is how a
    /// cached index comes to resolve a name the built one does not.
    pub(crate) fn from_cache(
        root: Option<PathBuf>,
        mut interner: Interner,
        mut types: TypeTable,
        file_count: usize,
    ) -> Self {
        // Re-derived rather than stored, and free: the table interns, so every one of these
        // nodes is already in what the cache handed back and asking for it again returns the same
        // id. Rebuilt rather than carried because an id means an index into *this* table.
        let superglobals = crate::superglobals::install(&mut types, &mut interner);
        Self {
            root,
            interner,
            types,
            superglobals,
            property_shapes: FxHashMap::default(),
            // Sized for what the cache says it holds. Grown by doubling from empty, the table for
            // thirty thousand files passed through fifteen sizes and freed each -- and macOS keeps a
            // freed region that large rather than returning it, so a warm server carried the last
            // few of them as dirty memory for its whole life.
            files: FxHashMap::with_capacity_and_hasher(file_count, Default::default()),
            owners: OwnerCatalog::default(),
            state: IndexState::Loading,
            // Empty because a cache holds what files said, and this is not that. Whoever loads one
            // installs it, through `adopt`.
            returns: ReturnContracts::default(),
            source_roots: Vec::new(),
            namespace_rules: Vec::new(),
            config_sites: Vec::new(),
            template_words: std::collections::HashSet::new(),
            twig_includes: Vec::new(),
            is_an_application: false,
            discovered_namespaces: Vec::new(),
            discovered_supertypes: Vec::new(),
            run_supertypes: Vec::new(),
            dynamic_supertypes: Vec::new(),
            own_method_prefixes: Vec::new(),
            stand_in_methods: Vec::new(),
            built_keys: Vec::new(),
            style: None,
            class_aliases: FxHashMap::default(),
            template_dialects: Vec::new(),
            keyed_attributes: KeyedAttributes::default(),
            flattened: Mutex::default(),
            reached: Mutex::default(),
            never_returning: Vec::new(),
            bound_scopes: Vec::new(),
            set_hooked: Vec::new(),
            attributes: Vec::new(),
            asserting_arguments: Vec::new(),
            receiver_out: Vec::new(),
            dispatches: FxHashMap::default(),
            declared_tables: FxHashMap::default(),
            stated: FxHashMap::default(),
            renders: FxHashMap::default(),
            configured_this: FxHashMap::default(),
            schemas: FxHashMap::default(),
            schema: std::sync::OnceLock::new(),
            class_defaults: FxHashMap::default(),
            constructions: FxHashMap::default(),
            macros: FxHashMap::default(),
            trait_users: FxHashMap::default(),
        }
    }

    /// Forgets every table built from any of these classes, because one of them just changed.
    fn forget_flattened_from(&self, changed: &[Name]) {
        if changed.is_empty() {
            return;
        }
        self.lock_flattened().tables.retain(|_, (table, _)| {
            !table.contributors().iter().any(|contributor| {
                changed
                    .iter()
                    .any(|name| name.as_str().eq_ignore_ascii_case(contributor.as_str()))
            })
        });
    }

    fn lock_reached(&self) -> std::sync::MutexGuard<'_, crate::Reached> {
        // Not poisoned for the reason the memo below is not: a contained panic mid-record leaves
        // one file's counts short, which the next analysis of that file corrects, where poisoning
        // would turn it into a second failure that never corrects.
        self.reached
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Takes on what analysing one file found it reaches.
    ///
    /// Under a *shared* lock on the workspace, like every other read: this is derived state, and
    /// the analysis that produced it was being done anyway -- see [`Reached`](crate::Reached).
    pub fn record_reached(&self, file: FileId, reaches: crate::Reaches) {
        self.lock_reached().record(file, reaches);
    }

    /// Whether anything in the project reaches this member of this class.
    ///
    /// Meaningless until [`Self::reached_is_complete`], which is what says every file has had its
    /// say: before that an absence means "not swept yet" and reading it as "dead" would report
    /// most of a project.
    pub fn member_is_reached(&self, owner: &str, name: &str) -> bool {
        self.lock_reached().holds(owner, name)
    }

    /// Whether anything outside its own file names this class -- see [`Reached`](crate::Reached).
    pub fn class_is_reached(&self, fqn: &str) -> bool {
        self.lock_reached().holds_class(fqn)
    }

    pub fn reached_is_complete(&self) -> bool {
        self.lock_reached().is_complete()
    }

    /// Said once a pass has covered every file the project owns.
    ///
    /// Answers whether this was the call that completed it, so the caller can republish the
    /// buffers whose answers just got sharper and not on every pass after.
    pub fn mark_reached_complete(&self) -> bool {
        self.lock_reached().mark_complete()
    }

    /// How many members, classes and loose names are held, for the memory report.
    pub fn reached_counts(&self) -> (usize, usize, usize) {
        self.lock_reached().counts()
    }

    fn lock_flattened(&self) -> std::sync::MutexGuard<'_, FlattenedMemo> {
        // A request that panicked while holding this left a map of whole tables behind, not a
        // half-written one: every insert is one `Arc`. Poisoning would only turn a contained panic
        // into a second failure.
        self.flattened
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Installs one row read from a cache, as `decode` reads it -- rather than after a whole
    /// vector of them has been read, which for a real project is a fourteen-megabyte allocation
    /// that lives for the length of the decode and then joins the freed regions the allocator keeps.
    pub(crate) fn install_cached(&mut self, file: FileId, entry: FileSkeleton, said: Said) {
        self.install(file, entry, said);
    }

    /// The reuse table, for the reader that fills it while a cache is decoded.
    pub(crate) fn interner_mut(&mut self) -> &mut Interner {
        &mut self.interner
    }

    /// Forgets every file except the ones listed.
    ///
    /// For a loaded index describing a checkout that has moved on: a file that was deleted has to
    /// leave the catalog with it, or its class keeps resolving to a path that is not there.
    /// Records what the disk said about a file whose rows were just read or confirmed from it.
    ///
    /// Separate from ingesting, because ingesting does not know where the text came from and the
    /// fingerprint is only true of text read from disk. A file the index does not hold is ignored.
    pub fn set_fingerprint(&mut self, file: FileId, fingerprint: Option<FileFingerprint>) {
        if let Some(entry) = self.files.get_mut(&file) {
            entry.fingerprint = fingerprint;
        }
    }

    pub fn retain_files(&mut self, live: &[FileId]) {
        // A set, not `live.contains`: that was a scan of thirty thousand ids for each of thirty
        // thousand files, 130 ms of every warm start spent confirming there was nothing to forget.
        let live: rustc_hash::FxHashSet<FileId> = live.iter().copied().collect();
        let stale = self
            .files
            .keys()
            .copied()
            .filter(|file| !live.contains(file))
            .collect::<Vec<_>>();
        for file in stale {
            self.remove(file);
        }
    }

    /// The project's type table. Every `TypeId` stored in a member row indexes into this.
    /// The type the engine gives a variable it binds itself, if this is one of them.
    pub fn superglobal_type(&self, name: &str) -> Option<phrost_ty::TypeId> {
        self.superglobals.get(name).copied()
    }

    /// Interns the `array{...}` every complete property table describes, and the list of it.
    ///
    /// Called from the two places the type table is established: after a framework's tables are
    /// installed, and at the end of [`Self::adopt`], which replaces the table an id indexes into.
    /// Missing either leaves ids pointing at a table that is gone -- the trap [`Self::adopt`]
    /// states for `superglobals` two lines from here, and the same answer.
    ///
    /// Only a complete table gets one: a shape claims that the names it lists are the names there
    /// are, and a partial table would state a type narrower than the truth. Such a table still
    /// answers for a property it holds, which is a different question and a different reader.
    pub fn intern_property_shapes(&mut self) {
        self.property_shapes.clear();
        // Owned, because the loop below holds the type table mutably and a borrow of `self`
        // would be the same borrow.
        let tables = self.returns.complete_tables();
        for (catalog, kind, properties) in tables {
            let fields: Vec<phrost_ty::TypeId> = properties
                .iter()
                .map(|(name, holds)| {
                    let ty = match holds {
                        Some(holds) => self.types.builtin(*holds),
                        // A property whose type the declaration does not state in PHP's own
                        // vocabulary. The key is there and nothing is claimed about it, which is
                        // what `mixed` says.
                        None => self.types.builtin(phrost_model::BuiltinTy::Mixed),
                    };
                    let name = self.interner.name(name);
                    self.types.field(name, false, ty)
                })
                .collect();
            // Sealed: the table is complete, so it names every property a row holds.
            let one = self.types.shape(&fields, false);
            let array = self.types.builtin(phrost_model::BuiltinTy::Array);
            let int = self.types.builtin(phrost_model::BuiltinTy::Int);
            let rows = self.types.generic(array, &[int, one]);
            self.property_shapes
                .insert((catalog, kind), PropertyShape { one, rows });
        }
    }

    /// The shape one key of `catalog` describes, through the kind it is of.
    ///
    /// The same two lookups [`ReturnContracts::property_table`] makes, and only for a table that
    /// says it is complete.
    pub fn property_shape(&self, catalog: &str, key: &str) -> Option<PropertyShape> {
        let kind = self.returns.catalog_kind(catalog, key)?;
        // Scanned rather than hashed on a borrowed pair, which would need an owned key made per
        // read. A few hundred rows, asked once per read of such a property.
        self.property_shapes
            .iter()
            .find(|((held, named), _)| &**held == catalog && &**named == kind)
            .map(|(_, shape)| *shape)
    }

    pub fn types(&self) -> &TypeTable {
        &self.types
    }

    /// Contracts a framework states about what a member returns.
    pub fn returns(&self) -> &ReturnContracts {
        &self.returns
    }

    /// Records that `class`'s `@method` tags were generated from `source`'s methods.
    pub fn add_tag_source(&mut self, class: &str, source: &str) {
        let fqn = self.interner.name(source);
        self.returns.add_tag_source(
            class,
            phrost_sema::ClassRef {
                fqn,
                range: phrost_model::TextRange::default(),
            },
        );
    }

    /// Records a `@mixin` a framework's class behaves as though it wrote -- `Builder<static>` when
    /// `with_static`, the class the call was made on binding the mixin's first template.
    pub fn add_implied_mixin(&mut self, class: &str, mixin: &str, with_static: bool) {
        let name = self.interner.name(mixin);
        let argument = with_static.then(|| {
            let base = self.types.named(name.clone());
            let own = self.types.relative(phrost_model::RelativeTy::Static);
            self.types.generic(base, &[own])
        });
        self.returns.add_implied_mixin(
            class,
            phrost_sema::ClassRef {
                fqn: name,
                range: phrost_model::TextRange::default(),
            },
            argument,
        );
    }

    /// For whoever knows which frameworks the project has. Nothing below this crate does.
    pub fn returns_mut(&mut self) -> &mut ReturnContracts {
        &mut self.returns
    }

    /// Takes on everything another skeleton derived from files, keeping this one's configuration.
    ///
    /// What a cache load and an index reset both need, and the reason they are not an assignment:
    /// replacing the whole struct silently discards anything installed from above, which is how the
    /// previous attempt came to serve a re-bootstrapped project without its framework knowledge --
    /// a failure with no error and no symptom except worse answers.
    ///
    /// Destructured rather than assigned field by field so that a field added later does not
    /// compile until somebody says which side of that line it is on.
    pub fn adopt(&mut self, other: Self) {
        let Self {
            root,
            interner,
            types,
            superglobals,
            // Interned into the table being replaced, so made again below rather than carried.
            property_shapes: _,
            files,
            owners,
            state,
            // Configuration: it describes the project, not what any file said, so what was loaded
            // has nothing to say about it.
            returns: _,
            style: _,
            class_aliases: _,
            template_dialects: _,
            source_roots: _,
            namespace_rules: _,
            config_sites: _,
            template_words: _,
            twig_includes: _,
            is_an_application: _,
            discovered_namespaces: _,
            discovered_supertypes: _,
            run_supertypes: _,
            dynamic_supertypes: _,
            own_method_prefixes: _,
            stand_in_methods: _,
            built_keys: _,
            keyed_attributes: _,
            // Built by requests against the old tables, so it goes with them.
            flattened: _,
            // Recorded by analysing the files being replaced, so likewise -- and it starts
            // incomplete, which is what stops a report speaking before the new tree is swept.
            reached: _,
            // Derived from the files, so they travel with them. A loaded index built both while
            // reading them back, which is why `install` is the one place a file enters the table.
            never_returning,
            bound_scopes,
            set_hooked,
            attributes,
            asserting_arguments,
            receiver_out,
            dispatches,
            declared_tables,
            stated,
            renders,
            configured_this,
            schemas,
            // Composed from the files, so composed again from the adopted ones.
            schema: _,
            class_defaults,
            constructions,
            macros,
            trait_users,
        } = other;

        self.lock_flattened().tables.clear();
        self.root = root;
        self.interner = interner;
        self.types = types;
        // With the table, never rebuilt: an id is an index into the table it was interned in, and
        // the one being adopted is not the one these were made from.
        self.superglobals = superglobals;
        self.files = files;
        self.never_returning = never_returning;
        self.bound_scopes = bound_scopes;
        self.set_hooked = set_hooked;
        self.attributes = attributes;
        self.asserting_arguments = asserting_arguments;
        self.receiver_out = receiver_out;
        self.dispatches = dispatches;
        self.declared_tables = declared_tables;
        self.stated = stated;
        self.renders = renders;
        self.configured_this = configured_this;
        self.schemas = schemas;
        self.schema = std::sync::OnceLock::new();
        self.class_defaults = class_defaults;
        self.constructions = constructions;
        self.macros = macros;
        self.trait_users = trait_users;
        self.owners = owners;
        self.state = state;

        // The keys the adopted files carry, claimed again into *this* skeleton's table.
        //
        // A loaded index claimed them into its own, and that table is the one thing not carried
        // over -- which is right, because the contracts saying *which* attribute is a key are
        // configuration and live here, installed before anything was loaded. But the claims are not
        // configuration, so they have to be made again. Skipping this leaves a warm start resolving
        // no entity type at all while a cold one resolves every one.
        //
        // Through [`Self::catalog_claims`] and not a reader chosen here, because this loop asked for
        // the declared keys by name and there are three kinds: a warm start held every key an
        // attribute named and none that a declaration spelled or that a function's own name carried.
        let claimed = self
            .files
            .iter()
            .map(|(file, entry)| (*file, self.catalog_claims(entry.scope())))
            .collect::<Vec<_>>();
        for (file, claims) in claimed {
            for claim in claims {
                claim.into_table(&mut self.returns, file);
            }
        }

        // And the extension points the adopted files dispatch, for the same reason: the rules
        // saying which method dispatches are configuration and live here, but what each file
        // dispatched is not, and the contract table holding the claims is the one thing not
        // carried over.
        let dispatched = self
            .dispatches
            .values()
            .flat_map(|rows| rows.iter().cloned())
            .collect::<Vec<_>>();
        for row in dispatched {
            let key = row.call.name.clone();
            self.returns
                .claim_dispatch(&row.catalog, key.as_str(), row.call);
        }

        // The tables the adopted files stated, put into *this* skeleton's contracts.
        //
        // The same reason the catalog claims above are made again, and the same trap: a loaded
        // index added them to its own `returns`, and `returns` is the one thing not carried over --
        // rightly, because the rules saying which declaration states a table are configuration and
        // live here. What the files *said* is not configuration, so it has to be said again. Warm
        // and cold answered differently for as long as this was missing, and only a driven server
        // could see it: `phrost query` and `phrost check` are both cold starts.
        let stated = self
            .declared_tables
            .values()
            .flat_map(|tables| tables.iter().cloned())
            .collect::<Vec<_>>();
        for table in stated {
            self.returns.add_property_table(
                &table.catalog,
                &table.kind,
                table.properties,
                table.complete,
            );
        }

        // Interned into the table this just replaced, so made again against the new one. The same
        // rule `superglobals` states above, arriving from the derived side. After the tables,
        // since it is those it walks.
        self.intern_property_shapes();
    }

    pub fn state(&self) -> IndexState {
        self.state
    }

    /// Declares that every discovered file has been ingested at least once.
    pub fn mark_ready(&mut self) {
        self.state = IndexState::Ready;
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn file(&self, file: FileId) -> Option<&FileSkeleton> {
        self.files.get(&file)
    }

    /// The extension points one file's own calls declare.
    pub fn dispatches_of(&self, file: FileId) -> &[DispatchRow] {
        self.dispatches.get(&file).map_or(&[], |rows| rows)
    }

    /// The property tables a file's own declarations state, for the codec that writes them out.
    pub fn declared_tables_of(&self, file: FileId) -> &[crate::tables::FoundTable] {
        self.declared_tables.get(&file).map_or(&[], |rows| rows)
    }

    /// The properties one file's classes state in an array they write.
    pub fn stated_of(&self, file: FileId) -> &[crate::StatedTable] {
        self.stated.get(&file).map_or(&[], |rows| rows)
    }

    /// The table `class_fqn` itself states, found through the file declaring it.
    /// What `file` says about the database schema -- see [`crate::schema`].
    pub fn schema_of(&self, file: FileId) -> Option<&crate::schema::FileSchema> {
        self.schemas.get(&file)
    }

    /// The tables the project's migrations leave, composed once per change to any of them.
    pub fn schema(&self) -> Arc<crate::schema::Schema> {
        Arc::clone(self.schema.get_or_init(|| Arc::new(self.compose_schema())))
    }

    /// The class `file`'s declaration of `class_fqn` names in the default of `property`.
    pub fn class_default(&self, file: FileId, class_fqn: &str, property: &str) -> Option<&str> {
        self.class_defaults
            .get(&file)?
            .iter()
            .find(|kept| {
                kept.declarer.eq_ignore_ascii_case(class_fqn) && &*kept.property == property
            })
            .map(|kept| &*kept.class)
    }

    /// Every macro `file` registers -- for the codec and the dump.
    pub fn macros_of(&self, file: FileId) -> &[crate::macros::MacroSite] {
        self.macros.get(&file).map_or(&[], |kept| kept)
    }

    /// Every registration of a macro, wherever it was made.
    pub fn macros(&self) -> impl Iterator<Item = &crate::macros::MacroSite> {
        self.macros.values().flat_map(|sites| sites.iter())
    }

    /// Every registration of a macro called `name`, wherever it was made, with the file it is in.
    pub fn macros_named<'s>(
        &'s self,
        name: &'s str,
    ) -> impl Iterator<Item = (FileId, &'s crate::macros::MacroSite)> + 's {
        self.macros.iter().flat_map(move |(file, sites)| {
            sites
                .iter()
                .filter(move |site| site.name.as_str().eq_ignore_ascii_case(name))
                .map(move |site| (*file, site))
        })
    }

    /// Every class a property default in `file` names -- for the codec and the dump.
    pub fn class_defaults_of(&self, file: FileId) -> &[crate::class_defaults::ClassDefault] {
        self.class_defaults.get(&file).map_or(&[], |kept| kept)
    }

    /// Every entry a construction in `file` declares -- for the codec and the dump.
    pub fn constructions_of(&self, file: FileId) -> &[crate::constructions::ConstructionSite] {
        self.constructions.get(&file).map_or(&[], |kept| kept)
    }

    /// Every entry of `catalog` some construction declares, with the file declaring it, in file
    /// order so two runs list them alike.
    pub fn constructions_in(
        &self,
        catalog: &str,
    ) -> Vec<(FileId, &crate::constructions::ConstructionSite)> {
        let mut found: Vec<_> = self
            .constructions
            .iter()
            .flat_map(|(file, sites)| sites.iter().map(move |site| (*file, site)))
            .filter(|(_, site)| *site.catalog == *catalog)
            .collect();
        found.sort_by_key(|(file, site)| (file.as_raw(), site.key_range.start()));
        found
    }

    /// The table `class_fqn`, declared in `file`, names outright.
    pub fn named_table(&self, file: FileId, class_fqn: &str) -> Option<&str> {
        self.schemas
            .get(&file)?
            .named
            .iter()
            .find(|named| named.class.eq_ignore_ascii_case(class_fqn))
            .map(|named| &*named.table)
    }

    pub fn stated_by(&self, file: FileId, class_fqn: &str) -> Option<&crate::StatedTable> {
        self.stated_of(file)
            .iter()
            .find(|table| table.declarer.eq_ignore_ascii_case(class_fqn))
    }

    /// The table the project's own code states for one kind of `catalog`, wherever it was declared.
    ///
    /// A scan: a project declares a few dozen kinds, and this is asked only once a property read
    /// through a list of that kind has been found at all.
    pub fn table_declaring(&self, catalog: &str, kind: &str) -> Option<&crate::tables::FoundTable> {
        self.declared_tables
            .values()
            .flat_map(|tables| tables.iter())
            .find(|table| &*table.catalog == catalog && &*table.kind == kind)
    }

    /// The table `class_fqn` itself states, found through the file declaring it -- a handful of
    /// entries at most, so no map keyed by class is kept beside the one keyed by file.
    pub fn table_declared_by(
        &self,
        file: FileId,
        class_fqn: &str,
    ) -> Option<&crate::tables::FoundTable> {
        self.declared_tables_of(file)
            .iter()
            .find(|table| table.declarer.eq_ignore_ascii_case(class_fqn))
    }

    /// Every class in the project that directly names `fqn` as a supertype or a trait, best first.
    ///
    /// A pass over the class table rather than a search of the files. The reverse of inheritance is
    /// not stored -- a class knows what it extends and nothing knows what extends it -- and storing
    /// it would be a table that grows with `vendor/`, which is the one thing this index is judged on.
    /// So it is computed, and computing it is affordable precisely because supertypes are already
    /// resolved: this compares interned names and reads no file at all.
    ///
    /// Ordered by origin and then by path, like every other list of declarers here, so two runs agree
    /// and a project's own classes come before a framework's.
    pub fn implementers_of(&self, fqn: &str) -> Vec<(FileId, &ClassSkeleton)> {
        let mut found = self
            .files
            .iter()
            .flat_map(|(file, entry)| {
                entry
                    .classes()
                    .iter()
                    .filter(|class| class.inherits_from(fqn))
                    .map(move |class| (*file, class))
            })
            .collect::<Vec<_>>();

        found.sort_by_key(|(file, class)| {
            let entry = self.files.get(file);
            (
                entry.map_or(u8::MAX, |entry| precedence(entry.origin, &entry.path)),
                entry.map(|entry| self.display_path(entry).to_owned()),
                class.fqn().as_str().to_owned(),
            )
        });
        found
    }

    /// The files that declare a class composing the trait `fqn`, by any spelling of its name.
    ///
    /// A superset in one direction only: two traits whose folded names collide share an entry,
    /// so a reader checks each file's classes for the trait it asked about. It never leaves a
    /// composing file out.
    pub fn files_using_trait(&self, fqn: &str) -> &[FileId] {
        self.trait_users
            .get(&Self::trait_key(fqn))
            .map_or(&[], Vec::as_slice)
    }

    /// Every entry of the trait map, for the dump that holds it to `incremental == from-scratch`:
    /// each trait by a name one of its files spells it with, and the files, in path order.
    pub(crate) fn trait_users(&self) -> Vec<(String, Vec<&FileSkeleton>)> {
        let mut entries = self
            .trait_users
            .iter()
            .filter_map(|(key, files)| {
                let files = files
                    .iter()
                    .filter_map(|file| self.files.get(file))
                    .collect::<Vec<_>>();
                let name = files
                    .iter()
                    .flat_map(|entry| entry.classes.iter())
                    .flat_map(|class| class.uses.iter())
                    .map(|used| used.fqn.as_str().to_ascii_lowercase())
                    .filter(|name| Self::trait_key(name) == *key)
                    .min()?;
                Some((name, files))
            })
            .collect::<Vec<_>>();
        for (_, files) in &mut entries {
            files.sort_by(|left, right| self.display_path(left).cmp(self.display_path(right)));
        }
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries
    }

    fn trait_key(fqn: &str) -> u64 {
        crate::words::WordSet::fold(fqn.trim_start_matches('\\'))
    }

    /// Every class in the project one of whose supertypes or traits `wanted` accepts.
    ///
    /// The set form of [`Self::implementers_of`], for a caller walking a hierarchy *downwards*.
    /// Taken one name at a time, a closure over a hierarchy costs one pass over the class table per
    /// class in it; this costs one per level, which for PHP is a handful whatever the project.
    ///
    /// Unordered, unlike its sibling: a walk collects across several levels and has to order the
    /// whole of what it found, so ordering each level would be work thrown away.
    pub fn implementers_matching(
        &self,
        mut wanted: impl FnMut(&str) -> bool,
    ) -> Vec<(FileId, &ClassSkeleton)> {
        let mut found = Vec::new();
        for (file, entry) in &self.files {
            for class in entry.classes() {
                if [class.extends(), class.implements(), class.uses()]
                    .into_iter()
                    .flatten()
                    .any(|named| wanted(named.fqn.as_str()))
                {
                    found.push((*file, class));
                }
            }
        }
        found
    }

    /// Records which namespace the autoloader maps each source directory onto.
    ///
    /// The prefix as well as the directory, because two questions are asked of the same fact and
    /// only one of them is about where code lives: whether a file's top level is private, and what
    /// namespace a file under that directory is *required* to declare. A rule with no prefix could
    /// answer the first and not the second.
    pub fn set_namespace_rules(&mut self, rules: Vec<(Box<str>, std::path::PathBuf)>) {
        self.namespace_rules = rules;
    }

    /// The autoload prefix composer maps onto `directory`, below the project's root: `App\` for
    /// `app`, which is how Laravel finds its application's namespace.
    pub fn namespace_of_directory(&self, directory: &str) -> Option<&str> {
        let wanted = self.root.as_ref()?.join(directory);
        self.namespace_rules
            .iter()
            .find(|(_, mapped)| mapped.components().eq(wanted.components()))
            .map(|(prefix, _)| &**prefix)
    }

    /// Records which namespaces hold classes a framework finds rather than is told about.
    pub fn set_discovered_namespaces(&mut self, segments: Vec<Box<str>>) {
        self.discovered_namespaces = segments;
    }

    pub fn set_is_an_application(&mut self, application: bool) {
        self.is_an_application = application;
    }

    /// Whether composer calls this package an application rather than a library.
    pub fn is_an_application(&self) -> bool {
        self.is_an_application
    }

    /// Adds to the list rather than replacing it: `install` runs once per detected provider, and
    /// two frameworks naming base classes are both right. The neighbour below replaces, which is a
    /// trap that sleeps only because one provider uses it today.
    pub fn add_run_supertypes(&mut self, supertypes: impl IntoIterator<Item = Box<str>>) {
        self.run_supertypes.extend(supertypes);
    }

    /// Drops what the providers contributed, for a re-read of the project's configuration.
    pub fn forget_run_supertypes(&mut self) {
        self.run_supertypes.clear();
        self.dynamic_supertypes.clear();
        self.stand_in_methods.clear();
        self.built_keys.clear();
    }

    pub fn add_dynamic_supertypes(&mut self, supertypes: impl IntoIterator<Item = Box<str>>) {
        self.dynamic_supertypes.extend(supertypes);
    }

    pub fn add_own_method_prefixes(&mut self, prefixes: impl IntoIterator<Item = Box<str>>) {
        self.own_method_prefixes.extend(prefixes);
    }

    pub fn add_stand_in_methods(&mut self, rules: impl IntoIterator<Item = StandInMethod>) {
        self.stand_in_methods.extend(rules);
    }

    /// The calls whose string argument names a method of the test double they are made on.
    pub fn stand_in_methods(&self) -> &[StandInMethod] {
        &self.stand_in_methods
    }

    pub fn add_built_keys(&mut self, rules: impl IntoIterator<Item = BuiltKey>) {
        self.built_keys.extend(rules);
    }

    /// The calls whose string argument names a key a sibling method writes into an array it builds.
    pub fn built_keys(&self) -> &[BuiltKey] {
        &self.built_keys
    }

    /// The method a string names when a framework's own spelling says it is one of this class's.
    ///
    /// `None` for a string that carries no such prefix, which is nearly every string in a project.
    pub fn own_method_named<'a>(&self, written: &'a str) -> Option<&'a str> {
        self.own_method_prefixes
            .iter()
            .find_map(|prefix| written.strip_prefix(&**prefix))
            .filter(|name| !name.is_empty())
    }

    /// Whether this class answers names no declaration holds -- see `dynamic_members`.
    pub fn answers_any_name(&self, fqn: &str) -> bool {
        self.inherits_any_of(fqn, &self.dynamic_supertypes)
    }

    pub fn set_discovered_supertypes(&mut self, supertypes: Vec<Box<str>>) {
        self.discovered_supertypes = supertypes;
    }

    /// Every class a framework builds because of what it inherits, fully qualified and lowercased.
    ///
    /// Walked from the ancestor down, which is the direction `implementers_of` answers, and
    /// repeated while the set grows so an intermediate base is not a wall. Bounded, because a
    /// cycle in a broken hierarchy is a thing a checkout can hold.
    pub fn discovered_by_inheritance(&self) -> std::collections::HashSet<String> {
        let mut found: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut asking: Vec<String> = self
            .discovered_supertypes
            .iter()
            .map(|name| name.to_string())
            .collect();
        for _ in 0..DISCOVERY_DEPTH {
            let mut next = Vec::new();
            for ancestor in asking.drain(..) {
                for (_, class) in self.implementers_of(&ancestor) {
                    let fqn = class.fqn().as_str().to_ascii_lowercase();
                    if found.insert(fqn.clone()) {
                        next.push(class.fqn().as_str().to_owned());
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            asking = next;
        }
        found
    }

    /// Whether a framework builds this class because of what it inherits.
    ///
    /// The per-class half of [`Self::discovered_by_inheritance`], which answers the same question
    /// by walking *down* from every discovered supertype -- a pass over the whole index, which a
    /// report can afford once and a keystroke cannot. Asked about one class it is a climb up that
    /// class's own ancestors, bounded by [`DISCOVERY_DEPTH`]: the `NeverReturns` shape, where what
    /// is expensive is the enumeration and nobody asking about one class needs it.
    pub fn inherits_a_discovered_supertype(&self, fqn: &str) -> bool {
        self.inherits_any_of(fqn, &self.discovered_supertypes)
    }

    /// Whether a framework runs this class and catches what it throws -- see `runs_and_catches`.
    pub fn is_run_by_a_framework(&self, fqn: &str) -> bool {
        self.inherits_any_of(fqn, &self.run_supertypes)
    }

    /// Whether `fqn` is, or inherits, any name in `supertypes`.
    ///
    /// One climb for both lists rather than one per list. The two questions a provider can answer
    /// with a set of base classes -- does the framework *build* this, does the framework *run* it
    /// -- are different questions about the same hierarchy, and §8.1's oldest form is two copies of
    /// one walk that learn a new case separately. The lists stay apart because what they mean does;
    /// the walk does not.
    fn inherits_any_of(&self, fqn: &str, supertypes: &[Box<str>]) -> bool {
        if supertypes.is_empty() {
            return false;
        }

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut asking = vec![fqn.trim_start_matches('\\').to_ascii_lowercase()];
        for _ in 0..DISCOVERY_DEPTH {
            let mut next = Vec::new();
            for name in asking.drain(..) {
                if supertypes
                    .iter()
                    .any(|supertype| supertype.eq_ignore_ascii_case(&name))
                {
                    return true;
                }
                let Some(class) = ClassSource::class(self, &name) else {
                    continue;
                };
                // Traits too: Drupal's own base classes are assembled from them, and a trait that
                // pulls in a discovered base is the same statement as extending it.
                for above in class
                    .extends
                    .iter()
                    .chain(class.implements)
                    .chain(class.uses)
                {
                    let above = above
                        .fqn
                        .as_str()
                        .trim_start_matches('\\')
                        .to_ascii_lowercase();
                    if seen.insert(above.clone()) {
                        next.push(above);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            asking = next;
        }
        false
    }

    /// Whether `fqn` sits in a namespace a framework scans, so nothing has to name it.
    ///
    /// A whole segment, not a substring: `App\Plugin\Foo` is one and `App\PluginHelper` is not.
    pub fn is_discovered(&self, fqn: &str) -> bool {
        self.discovered_namespaces.iter().any(|segment| {
            fqn.split('\\')
                .any(|part| part.eq_ignore_ascii_case(segment))
        })
    }

    /// Records where the project's configuration names a class.
    ///
    /// Set rather than added to, like the namespace rules: the whole set is derived from one pass
    /// over the configuration, so a second pass replaces it and a stale half cannot survive.
    pub fn set_template_words(&mut self, words: std::collections::HashSet<Box<str>>) {
        self.template_words = words;
    }

    /// Whether a template writes this name, under any of the spellings Twig reaches it by.
    ///
    /// Lowercased on both sides, because PHP looks a method up that way and a template writes
    /// whatever casing the author typed.
    pub fn a_template_writes(&self, name: &str) -> bool {
        self.template_words
            .contains(name.to_ascii_lowercase().as_str())
    }

    /// Records the names one template renders other templates by, replacing what it said before.
    pub fn set_twig_includes(&mut self, template: &Path, names: Vec<Box<str>>) {
        self.twig_includes
            .retain(|(including, _)| &**including != template);
        let template: std::sync::Arc<Path> = template.into();
        self.twig_includes.extend(
            names
                .into_iter()
                .map(|name| (std::sync::Arc::clone(&template), name)),
        );
    }

    /// The file a template name loads, by the catalogs a Twig loader named: its names, then its
    /// aliases.
    pub fn twig_path_of(&self, name: &str) -> Option<&Path> {
        let returns = self.returns();
        returns.twig_loaders().iter().find_map(|loader| {
            returns
                .catalog_placed_at(&loader.templates, name)
                .or_else(|| returns.catalog_placed_at(&loader.aliases, name))
                .map(|place| &*place.path)
        })
    }

    /// Every template of the project's own that renders the one at `path` by a name it writes,
    /// each once.
    pub fn twig_includers_of(&self, path: &Path) -> Vec<&Path> {
        let mut found: Vec<&Path> = Vec::new();
        for (including, name) in &self.twig_includes {
            if &**including != path
                && self.twig_path_of(name) == Some(path)
                && !found.contains(&&**including)
            {
                found.push(including);
            }
        }
        found
    }

    pub fn set_config_sites(&mut self, sites: Vec<crate::ConfigSite>) {
        self.config_sites = sites;
    }

    /// Every place the configuration writes `class::member`, which is how a route names a
    /// controller's action.
    ///
    /// Matched on the class as written. A route naming a subclass whose action is declared in a
    /// parent is not found, which under-reports rather than rewrites the wrong line -- and a
    /// controller declaring its own actions, which is what they do, is found.
    pub fn config_sites_calling<'a>(
        &'a self,
        class: &'a str,
        member: &'a str,
    ) -> impl Iterator<Item = (&'a crate::ConfigSite, phrost_model::TextRange)> + 'a {
        self.config_sites_naming(class).filter_map(move |site| {
            let (written, at) = site.member.as_ref()?;
            written.eq_ignore_ascii_case(member).then_some((site, *at))
        })
    }

    /// Every class the configuration names, once each is not promised -- a caller collecting them
    /// into a set gets that for free.
    pub fn config_named_classes(&self) -> impl Iterator<Item = &str> + '_ {
        self.config_sites
            .iter()
            .map(|site| &*site.name)
            // A site that names a member and no class -- a container `factory`'s bare method --
            // says nothing about any class, and an empty name would match one only by accident.
            .filter(|name| !name.is_empty())
    }

    /// Whether the configuration writes this member name anywhere.
    ///
    /// The per-name half of [`Self::config_named_members`], for a reader judging one declaration
    /// rather than collecting the lot: a route's `_controller`, a service's `factory` method.
    /// Matched on the name alone, as the rest of this rule is -- see `config_sites_calling` for
    /// the paired question, which a reader that already knows the class asks instead.
    /// Whether something that is not a call written in PHP reaches this member.
    ///
    /// Three sources in one question, because asking it in two places is how the editor's
    /// `unused-declaration` and the `phrost unused` report came to disagree about 46 findings on
    /// one real project: the report had the framework's discovery and the configuration and not
    /// the template, while its own module doc listed all three. Nothing fails when one of two
    /// assemblies of a rule is missing a case, which is why there is one assembly now.
    pub fn member_is_reached_without_a_call(&self, owner: &str, name: &str) -> bool {
        self.is_discovered(owner)
            || self.inherits_a_discovered_supertype(owner)
            || self.config_names_member(name)
            || self.a_template_writes(name)
    }

    pub fn config_names_member(&self, name: &str) -> bool {
        self.config_sites.iter().any(|site| {
            site.member
                .as_ref()
                .is_some_and(|(written, _)| written.eq_ignore_ascii_case(name))
        })
    }

    /// Every member name the configuration writes, lowercased.
    pub fn config_named_members(&self) -> impl Iterator<Item = String> + '_ {
        self.config_sites
            .iter()
            .filter_map(|site| site.member.as_ref())
            .map(|(name, _)| name.to_ascii_lowercase())
    }

    /// Every place the configuration writes `fqn`, which is every reference to it outside PHP.
    pub fn config_sites_naming<'a>(
        &'a self,
        fqn: &'a str,
    ) -> impl Iterator<Item = &'a crate::ConfigSite> + 'a {
        let fqn = fqn.trim_start_matches('\\');
        self.config_sites
            .iter()
            // Case-insensitively, as PHP resolves a class name and as the rest of this index does.
            .filter(move |site| site.name.eq_ignore_ascii_case(fqn))
    }

    /// Every namespace the autoloader would accept for a file at `path`, most specific first.
    ///
    /// More than one, because PSR-4 roots overlap and composer does not pick between them by
    /// directory. It resolves a *class name*: the longest registered **prefix** the name starts with
    /// decides which directory is looked in, so a file is reachable if **any** rule's prefix and
    /// directory together produce its path. Symfony registers
    /// `Symfony\Runtime\Symfony\Component\ => src/Symfony/Component/Runtime/Internal/` *inside*
    /// `Symfony\Component\ => src/Symfony/Component/`, and its `Runtime/Internal/` files declare
    /// the namespace the second rule wants. Reading only the longest directory reported four of
    /// them as unfindable, which is a claim about composer rather than about the code.
    ///
    /// Most specific first so a caller wanting one answer -- a message suggesting what to declare --
    /// takes the first and gets the same one this used to return.
    ///
    /// Empty for a file under no source root at all -- a script, a template, something in a
    /// classmap. The autoloader never reaches those by name, so there is nothing they must be
    /// called.
    pub fn permitted_namespaces(&self, path: &std::path::Path) -> Vec<String> {
        // The autoload map's directories are absolute and a file's stored path is relative to the
        // project, which is a difference that fails as silence: every `starts_with` answers no and
        // the whole check reports nothing on a project where it should report two files.
        let absolute;
        let path = match path.is_absolute() {
            true => path,
            false => match self.root() {
                Some(root) => {
                    absolute = root.join(path);
                    &absolute
                }
                None => return Vec::new(),
            },
        };

        let Some(parent) = path.parent() else {
            return Vec::new();
        };
        let mut matching: Vec<_> = self
            .namespace_rules
            .iter()
            .filter(|(_, root)| lies_under(path, root))
            .collect();
        matching.sort_by_key(|(_, root)| std::cmp::Reverse(root.as_os_str().len()));

        matching
            .into_iter()
            .filter_map(|(prefix, root)| {
                let mut namespace = prefix.trim_end_matches('\\').to_owned();
                for part in parent.strip_prefix(root).ok()?.components() {
                    namespace.push('\\');
                    namespace.push_str(&part.as_os_str().to_string_lossy());
                }
                Some(namespace)
            })
            .collect()
    }

    /// Records the directories the autoloader maps a namespace onto.
    ///
    /// Configuration, installed once by whoever read composer's map. What it settles is one question
    /// nothing else can: an autoloaded file is reached through `includeFile()`, which is a *function*,
    /// so its top-level variables are local to that call and nobody outside can read or write them.
    /// Every other file is reached by an `include` written somewhere, and there the top level belongs
    /// to whoever wrote it.
    pub fn set_source_roots(&mut self, roots: Vec<std::path::PathBuf>) {
        self.source_roots = roots;
    }

    /// Whether the file's own top level is a scope nothing outside it can share.
    ///
    /// Both halves are needed and neither is about a framework. Under a source root is the
    /// autoloader's territory; declaring a class-like is what makes the autoloader the thing that
    /// loads it -- `src/bootstrap.php` sits under the same root and is `require`d by hand, and its
    /// variables are the caller's.
    pub fn file_scope_is_private(&self, file: FileId) -> bool {
        let Some(entry) = self.file(file) else {
            return false;
        };
        if entry.classes().is_empty() {
            return false;
        }
        let path = entry.path();
        self.source_roots.iter().any(|root| lies_under(path, root))
    }

    /// Records which attributes lowering keeps a positional string literal for.
    ///
    /// Before anything is ingested, like the contracts: a file lowered without the list carries no
    /// literal for it, and re-ingesting it later is what it would take to notice.
    pub fn set_keyed_attributes(&mut self, attributes: KeyedAttributes) {
        self.keyed_attributes = attributes;
    }

    /// The same list, for whoever else lowers a file of this project.
    ///
    /// An open buffer is lowered by the open set rather than here, and the two must agree or opening
    /// a file withdraws the keys it declares -- which is the `open == closed` invariant failing in
    /// the one direction nothing else looks at.
    pub fn keyed_attributes(&self) -> &KeyedAttributes {
        &self.keyed_attributes
    }

    /// Records that a file whose name ends in `suffix` is a template, read as what `to_php` makes
    /// of it.
    pub fn add_template_dialect(&mut self, dialect: TemplateDialect) {
        self.template_dialects
            .retain(|known| !known.suffix.eq_ignore_ascii_case(&dialect.suffix));
        self.template_dialects.push(dialect);
    }

    /// The name a call renders `path` by: Laravel's `posts.show` for
    /// `resources/views/posts/show.blade.php`.
    pub fn template_name(&self, path: &Path) -> Option<String> {
        let name = path.file_name()?.to_str()?;
        let dialect = self.template_dialects.iter().find(|dialect| {
            name.len() > dialect.suffix.len() && name.ends_with(&*dialect.suffix)
        })?;
        let within = path
            .strip_prefix(self.root.as_ref()?)
            .ok()?
            .strip_prefix(&*dialect.directory)
            .ok()?
            .to_str()?;
        let within = within.strip_suffix(&*dialect.suffix)?;
        Some(
            within
                .split(std::path::MAIN_SEPARATOR)
                .collect::<Vec<_>>()
                .join(&dialect.separator),
        )
    }

    /// Whether the dialect reading the template at `path` writes calls to `function`, which no file
    /// declares.
    pub fn template_writes(&self, path: &Path, function: &str) -> bool {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        self.template_dialects
            .iter()
            .find(|dialect| name.len() > dialect.suffix.len() && name.ends_with(&*dialect.suffix))
            .is_some_and(|dialect| {
                dialect
                    .calls
                    .iter()
                    .any(|call| call.eq_ignore_ascii_case(function))
            })
    }

    /// What in the structure of the template at `path`, whose text is `text`, its PHP cannot carry.
    /// Nothing for a file that is no template.
    pub fn template_problems(
        &self,
        path: &Path,
        text: &str,
    ) -> Vec<(std::ops::Range<usize>, String)> {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Vec::new();
        };
        self.template_dialects
            .iter()
            .find(|dialect| name.len() > dialect.suffix.len() && name.ends_with(&*dialect.suffix))
            .map_or_else(Vec::new, |dialect| (dialect.problems)(text))
    }

    /// What the name of the template at `path` writes between directories.
    pub fn template_separator(&self, path: &Path) -> Option<&str> {
        let name = path.file_name()?.to_str()?;
        self.template_dialects
            .iter()
            .find(|dialect| name.len() > dialect.suffix.len() && name.ends_with(&*dialect.suffix))
            .map(|dialect| &*dialect.separator)
    }

    /// Whether `path` is a template some framework reads as PHP.
    pub fn is_template(&self, path: &Path) -> bool {
        self.template_dialect(path).is_some()
    }

    /// The text `path` is read as: itself, or for a template the PHP its dialect makes of it.
    ///
    /// Every place a file's text enters the index goes through here -- a read from disk, a batch,
    /// an open buffer -- because a template read as markup in one place and as PHP in another is
    /// a file the open and closed tiers describe differently. The PHP is as long as the template
    /// and keeps its newlines, so a position means the same thing in both and nothing translates
    /// one.
    pub fn as_php(&self, path: &Path, text: &str) -> Option<String> {
        let to_php = self.template_dialect(path)?;
        Some(to_php(text).0)
    }

    fn template_dialect(&self, path: &Path) -> Option<TemplateToPhp> {
        template_dialect(&self.template_dialects, path)
    }

    /// The variables every template at `path` has.
    pub fn template_shared_names(&self, path: &Path) -> Vec<&str> {
        self.shared_for(path).map(|(name, _)| name).collect()
    }

    /// The class a variable every template at `path` has holds, when one does.
    pub fn template_shared(&self, path: &Path, name: &str) -> Option<&str> {
        self.shared_for(path)
            .find(|(variable, _)| *variable == name)
            .map(|(_, class)| class)
    }

    /// What every template at `path` has, and -- for a component's -- what every component has.
    fn shared_for(&self, path: &Path) -> impl Iterator<Item = (&str, &str)> {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let dialect = self.template_dialects.iter().find(|dialect| {
            file_name.len() > dialect.suffix.len() && file_name.ends_with(&*dialect.suffix)
        });
        let name = self.template_name(path);
        dialect
            .into_iter()
            .flat_map(move |dialect| {
                let own = dialect
                    .components
                    .iter()
                    .find(|(prefix, _)| {
                        name.as_deref()
                            .is_some_and(|name| name.starts_with(&**prefix))
                    })
                    .map_or(&[][..], |(_, shared)| &shared[..]);
                dialect.shared.iter().chain(own)
            })
            .map(|(name, class)| (&**name, &**class))
    }

    /// The dialects, for a batch that reads files away from the skeleton.
    pub(crate) fn template_dialects(&self) -> Vec<TemplateDialect> {
        self.template_dialects.clone()
    }

    /// Records that the framework's loader gives `class` the global name `alias` at start.
    pub fn add_class_alias(&mut self, alias: &str, class: &str) {
        self.class_aliases.insert(
            alias.trim_start_matches('\\').to_ascii_lowercase().into(),
            class.trim_start_matches('\\').into(),
        );
    }

    /// Every file's claim on a class-like name -- or, where no file declares it, on the class a
    /// framework's loader aliases it to.
    ///
    /// The one place a class is looked up by name, so that an alias answers wherever a declaration
    /// would. Not a fallback: the alias is as much a fact of the project as a declaration is, and a
    /// name declared and aliased at once is the declaration's, as it is in PHP, whose `class_alias`
    /// refuses a name already taken.
    pub fn class_declarers(&self, fqn: &str) -> &[Declarer] {
        let fqn = fqn.trim_start_matches('\\');
        let declared = self
            .interner
            .lookup_key(fqn)
            .map_or(&[][..], |key| self.owners.class_like(&key));
        if !declared.is_empty() || self.class_aliases.is_empty() {
            return declared;
        }
        self.class_aliases
            .get(fqn.to_ascii_lowercase().as_str())
            .and_then(|class| self.interner.lookup_key(class))
            .map_or(&[][..], |key| self.owners.class_like(&key))
    }

    /// Records the house style, by the name a framework gave it.
    pub fn set_house_style(&mut self, style: Option<&str>) {
        self.style = style.map(Into::into);
    }

    /// The house style's name, for whoever knows what the names mean.
    pub fn house_style(&self) -> Option<&str> {
        self.style.as_deref()
    }

    pub fn owners(&self) -> &OwnerCatalog {
        &self.owners
    }

    /// The short form of a file's path: relative to the project root when there is one.
    ///
    /// What a person reads in a hover, and what conflict ranking compares. An absolute path would
    /// fill a hover with a home directory and make ranking depend on the checkout location.
    pub fn display_path<'a>(&self, file: &'a FileSkeleton) -> &'a str {
        let path = self
            .root
            .as_ref()
            .and_then(|root| file.path.strip_prefix(root).ok())
            .unwrap_or(&file.path);

        path.to_str().unwrap_or("<non-utf8 path>")
    }

    /// The short path of a file by id.
    pub fn display_path_of(&self, file: FileId) -> Option<&str> {
        Some(self.display_path(self.files.get(&file)?))
    }

    /// Reads a slice of a file from disk.
    ///
    /// Used for text the skeleton deliberately does not retain — docblocks above all. Keeping every
    /// docblock of every vendor class resident is tens of megabytes on a real project, and reading
    /// one file to answer one hover is well inside the budget. The read is not a fallback for a
    /// missing index answer; it is where this particular fact was always going to come from.
    ///
    /// Returns `None` when the file cannot be read or the range does not land on character
    /// boundaries, which is what a stale range looks like.
    pub fn text_slice(&self, file: FileId, range: TextRange) -> Option<String> {
        let entry = self.files.get(&file)?;
        let source = SourceText::read(&entry.path).ok()?;

        source
            .text()
            .get(usize::from(range.start())..usize::from(range.end()))
            .map(str::to_owned)
    }

    /// Every file, for dumping. The caller sorts, because the map order is not stable.
    pub(crate) fn files_by_path(&self) -> Vec<(FileId, &FileSkeleton)> {
        self.files
            .iter()
            .map(|(&file, entry)| (file, entry))
            .collect()
    }

    pub fn interner(&self) -> &Interner {
        &self.interner
    }

    /// Parses, lowers, and ingests one file, replacing anything previously held for it.
    ///
    /// The unit of work is one file. Re-ingesting it touches only its own entries -- literally, by
    /// name -- so an edit costs what the file costs and not what the project costs.
    /// `path` must be absolute. It is what reading the file back needs -- a docblock is fetched on
    /// demand from a range in it -- and the short form a person reads is derived from the root
    /// instead. Passing a relative path here reads as working until something asks for text that was
    /// not retained, and then fails silently for every closed file.
    pub fn ingest(&mut self, file: FileId, path: &Path, origin: SourceOrigin, text: &str) {
        debug_assert!(
            path.is_absolute(),
            "ingest needs an absolute path, got {}",
            path.display()
        );
        // Bodies for project code only. A class declared inside a body -- `new class { ... }` above
        // all -- exists nowhere else, so a declaration-only lowering leaves the index with no facts
        // about it at all: `$this` inside its methods names a class the catalog has never heard of,
        // and every member reached through it is unresolvable. The bodies themselves are dropped
        // with the HIR at the end of this function; what survives is the declarations they held.
        //
        // Not for vendor or stubs, which is the whole memory rule: an anonymous class there can only
        // ever be named from inside its own file, and nobody puts a cursor in one.
        let lowered = if origin.wants_full_analysis() {
            phrost_lower::lower_text_with_bodies(
                &mut self.interner,
                file,
                text,
                &self.keyed_attributes,
            )
        } else {
            phrost_lower::lower_text(&mut self.interner, file, text, &self.keyed_attributes)
        };
        self.ingest_lowered(
            file,
            path,
            origin,
            text,
            FileStamp::of(text.as_bytes()),
            &lowered.hir,
        );
    }

    /// Ingests a file whose HIR the caller already lowered.
    ///
    /// Opening a buffer produces exactly the HIR this needs, and lowering the same text a second
    /// time to claim its declarations is the whole cost of opening a file twice over. The HIR may
    /// come from another interner: a `Name` compares by content, and the scope is rebuilt against
    /// this one, so nothing crosses that carries a key.
    ///
    /// Only sound for an origin that would have been body-lowered anyway -- see [`Self::ingest`].
    /// Handing a body-lowered vendor HIR to this would claim declarations the memory rule says the
    /// index never looks for.
    ///
    /// `stamp` is the caller's, because every caller already has it: a `SourceText` hashes its
    /// bytes when it is read. Hashing them again here was 104 ms of a 2.9 s cold start, and on the
    /// one thread that claims what the workers lowered -- nearly half of what that thread does.
    pub fn ingest_lowered(
        &mut self,
        file: FileId,
        path: &Path,
        origin: SourceOrigin,
        text: &str,
        stamp: FileStamp,
        hir: &phrost_hir::FileHir,
    ) {
        debug_assert!(
            path.is_absolute(),
            "ingest needs an absolute path, got {}",
            path.display()
        );
        let scope = FileScope::build(&mut self.interner, hir);
        // Read from the body, which exists here and nowhere else: a warm start never lowers, so
        // what a bound closure said has to travel through the codec like the rest of it.
        let bound_scopes: Vec<Name> = phrost_body::bound_scope_names(hir)
            .iter()
            .map(|short| self.interner.name(short))
            .collect();
        let classes = Self::collect_classes(&mut self.interner, &mut self.types, &scope, hir);
        let functions = Self::collect_functions(&mut self.interner, &mut self.types, &scope, hir);
        // Bodies, so only a file that was body-lowered contributes -- which is the project's own
        // code and nothing else. Derived here rather than in `install`, because this is the one
        // path that still has the HIR: a file read back from a cache has none, so what it declared
        // travels through the codec instead.
        let dispatches =
            Self::collect_dispatches(&self.returns, &mut self.interner, file, &scope, text, hir);
        // Same window and the same reason: the body is here and nowhere else, and what it said has
        // to travel through the codec because a warm start never lowers anything.
        let tables = Self::collect_declared_tables(&self.returns, &scope, text, hir);
        // The same window: a property's kept default and a method's body are here and nowhere else.
        let stated = Self::collect_stated(
            &self.returns,
            &mut self.interner,
            &mut self.types,
            &scope,
            text,
            hir,
        );
        let renders = Self::collect_renders(&self.returns, &scope, text, hir);
        let configured_this =
            Self::collect_configured_this(&self.returns, &mut self.interner, &scope, text, hir);
        let schema =
            Self::collect_schema(&self.returns, &mut self.interner, &scope, path, text, hir);
        let class_defaults = Self::collect_class_defaults(&scope, hir);
        let constructions = Self::collect_constructions(&self.returns, &scope, hir);
        let macros = Self::collect_macros(
            &self.returns,
            &mut self.interner,
            &mut self.types,
            &scope,
            text,
            hir,
        );
        let throws = Self::collect_throws(&mut self.interner, &scope, hir);
        let returned_paths = Self::collect_returned_paths(&mut self.interner, &scope, hir);
        let constants =
            crate::constant::ConstantRow::collect(&mut self.interner, &mut self.types, &scope, hir);

        // Remove first, unconditionally: a symbol the file no longer declares has to disappear,
        // and an update that only adds would leave the old name resolving to a stale location.
        // Claiming is `install`'s, so that the two paths a file can enter the table by cannot
        // disagree about what a file entering it means.
        self.retract(file);

        self.install(
            file,
            FileSkeleton {
                origin,
                stamp,
                // Set by the caller that read the file, if one did: this path is also how an
                // editor's buffer enters the index, and a buffer has no metadata worth trusting.
                fingerprint: None,
                // Retained only to break FQN conflicts deterministically -- see `rank`.
                path: Arc::from(path),
                scope,
                classes,
                functions,
                // The project's own code only, which is the same set a sweep reads.
                words: matches!(origin, SourceOrigin::User)
                    .then(|| crate::words::WordSet::of(text)),
                bound_scopes: bound_scopes.into_boxed_slice(),
                strict_types: hir.strict_types(),
                // Docblocks, which the HIR has and a cache-read file does not -- so this is derived
                // on the one path that can and travels through the codec on the other, exactly as
                // `dispatches` below does.
                throws,
                returned_paths,
                constants,
            },
            Said {
                dispatches,
                tables,
                stated,
                renders,
                configured_this,
                schema,
                class_defaults,
                constructions,
                macros,
            },
        );
    }

    /// The one place a file enters the table, whether it was just lowered or just read off a cache.
    ///
    /// One place because everything derived from a file's rows has to be derived on both paths, and
    /// two call sites is how one of them comes to be forgotten -- the shape §8 records for the
    /// lowering gate and for the catalog's keys. `claim` was already duplicated across the two.
    fn install(&mut self, file: FileId, mut entry: FileSkeleton, said: Said) {
        let Said {
            dispatches,
            tables,
            stated,
            renders,
            configured_this,
            schema,
            class_defaults,
            constructions,
            macros,
        } = said;
        // A dependency's imports have done their work by now -- everything the file declares was
        // resolved against them on the way here -- and nothing reads a closed file's imports again.
        // Kept for the project's own files, which are the ones that get opened and edited.
        if !entry.origin.wants_full_analysis() {
            entry.scope.forget_imports();
        }
        let declared: Vec<Name> = entry
            .classes
            .iter()
            .map(|class| class.fqn.clone())
            .collect();
        self.forget_flattened_from(&declared);
        for name in entry.bound_scopes.iter() {
            self.note_bound_scope(&name.clone());
        }
        for used in entry.classes.iter().flat_map(|class| class.uses.iter()) {
            let files = self
                .trait_users
                .entry(Self::trait_key(used.fqn.as_str()))
                .or_default();
            if let Err(at) = files.binary_search(&file) {
                files.insert(at, file);
            }
        }
        for class in &entry.classes {
            if class.is_attribute() {
                let short = phrost_model::short_name(class.fqn.as_str()).to_owned();
                let key = self.interner.key(&short);
                if let Err(at) = self.attributes.binary_search(&key) {
                    self.attributes.insert(at, key);
                }
            }
            // Whether a `set` hook this class declares could be an override of somebody else's
            // property -- see [`Self::set_hooked`].
            let could_override =
                !class.extends.is_empty() || class.kind == phrost_sema::ClassKind::Trait;
            for member in &class.members {
                if member.kind == MemberKind::Method
                    && (self.returns_never(member.ty, member.doc_ty)
                        || self.contract_returns_never(member.contract.as_deref()))
                {
                    self.note_never_returning(&member.name);
                }
                if member.kind == MemberKind::Method
                    && matches!(
                        member.contract.as_deref(),
                        Some(phrost_sema::Contract::ReceiverOut { .. })
                    )
                {
                    self.note_receiver_out(&member.name);
                }
                if member.modifiers.hooks == phrost_sema::Hooks::Settable && could_override {
                    self.note_set_hooked(&member.name);
                }
            }
        }
        for function in &entry.functions {
            // The short name, because that is what a call writes. A free function's row carries an
            // index into the file's declared symbols rather than a name of its own.
            let never = self.returns_never(function.ty(), function.doc_ty())
                || self.contract_returns_never(function.contract());
            let asserts = Self::asserts_about_an_argument(function.contract());
            if (never || asserts)
                && let Some(symbol) = entry.scope.symbols().get(function.symbol() as usize)
            {
                let short = phrost_model::short_name(symbol.fqn.as_str()).to_owned();
                let name = self.interner.name(&short);
                if never {
                    self.note_never_returning(&name);
                }
                if asserts {
                    self.note_asserting_arguments(&name);
                }
            }
        }

        for row in &dispatches {
            self.returns
                .claim_dispatch(&row.catalog, row.call.name.as_str(), row.call.clone());
        }
        if !dispatches.is_empty() {
            self.dispatches.insert(file, dispatches);
        }
        if !tables.is_empty() {
            for table in &tables {
                self.returns.add_property_table(
                    &table.catalog,
                    &table.kind,
                    table.properties.clone(),
                    table.complete,
                );
            }
            self.declared_tables.insert(file, tables);
            // Every table again, because interning is what a shape is: one file adding one is a
            // few hundred rows re-walked, and on a real checkout it happens once per declaring
            // file -- seventy-three times against thirty thousand ingests. Folded in here rather
            // than left to a caller, because a second place to remember is a place to forget.
            self.intern_property_shapes();
        }

        if !stated.is_empty() {
            self.stated.insert(file, stated);
        }
        if !renders.is_empty() {
            self.renders.insert(file, renders);
        }
        if !configured_this.is_empty() {
            self.configured_this.insert(file, configured_this);
        }
        if !schema.is_empty() {
            self.schemas.insert(file, schema);
            self.schema = std::sync::OnceLock::new();
        }
        if !constructions.is_empty() {
            self.constructions.insert(file, constructions);
        }
        if !class_defaults.is_empty() {
            self.class_defaults.insert(file, class_defaults);
        }
        if !macros.is_empty() {
            self.macros.insert(file, macros);
        }

        self.claim(file, entry.scope());
        self.files.insert(file, entry);
    }

    /// Whether either of a declaration's two return types is `never`.
    ///
    /// Both, because the docblock's is ranked against the signature's rather than merged with it,
    /// and `@return never` on an unannotated method is how this is written before PHP 8.1.
    fn returns_never(
        &self,
        ty: Option<phrost_ty::TypeId>,
        doc_ty: Option<phrost_ty::TypeId>,
    ) -> bool {
        [ty, doc_ty].into_iter().flatten().any(|id| {
            matches!(
                self.types.get(id),
                phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::Never)
            )
        })
    }

    /// Whether a branch of a declaration's contract is `never`.
    ///
    /// WordPress's `wp_die()` is `($args is array{exit: false} ? void : never)`: whether it returns is
    /// decided at the call, so the declaration says neither, and only the contract can put the name
    /// on the list. It passed on WordPress by coincidence -- a customizer method of the same name
    /// declares `@return never` -- and nowhere else.
    fn contract_returns_never(&self, contract: Option<&phrost_sema::Contract>) -> bool {
        let never = |branch: &phrost_sema::ReturnBranch| match branch {
            phrost_sema::ReturnBranch::Ty(id) => self.returns_never(Some(*id), None),
            _ => false,
        };
        match contract {
            Some(phrost_sema::Contract::ConditionalReturn {
                then, otherwise, ..
            }) => never(then) || never(otherwise),
            Some(phrost_sema::Contract::TestChain { cases, otherwise }) => {
                never(otherwise) || cases.iter().any(|case| never(&case.then))
            }
            Some(phrost_sema::Contract::LiteralCases {
                cases, otherwise, ..
            }) => never(otherwise) || cases.iter().any(|(_, branch)| never(branch)),
            _ => false,
        }
    }

    /// Whether a declaration's contract asserts something about one of its arguments, per branch.
    ///
    /// Only the suffixed forms. An unsuffixed `@phpstan-assert` holds unconditionally and is applied
    /// where the call is written, by the call-statement walk -- it never needs a guard to be read
    /// through, so putting its name here would gate nothing and cost a lookup.
    fn asserts_about_an_argument(contract: Option<&phrost_sema::Contract>) -> bool {
        matches!(
            contract,
            Some(phrost_sema::Contract::Argument {
                when_true: Some(_),
                ..
            })
        )
    }

    fn note_asserting_arguments(&mut self, name: &Name) {
        if !self
            .asserting_arguments
            .iter()
            .any(|known| known.as_str().eq_ignore_ascii_case(name.as_str()))
        {
            self.asserting_arguments.push(name.clone());
        }
    }

    /// Whether anything in the project declares a *function* of this name whose contract asserts
    /// about an argument. See [`Self::asserting_arguments`].
    ///
    /// Functions only. A method's argument assertion is reached through a root that names the
    /// method and the class it was found on, which the guard reader builds from the receiver it
    /// already has -- there is no name to guess and so nothing to gate.
    pub fn declares_argument_assertion(&self, name: &str) -> bool {
        self.asserting_arguments
            .iter()
            .any(|known| known.as_str().eq_ignore_ascii_case(name))
    }

    fn note_bound_scope(&mut self, name: &Name) {
        if !self
            .bound_scopes
            .iter()
            .any(|known| known.as_str().eq_ignore_ascii_case(name.as_str()))
        {
            self.bound_scopes.push(name.clone());
        }
    }

    /// Whether any file but `file` could hold `name`, asked of the word sets.
    ///
    /// The cheap half of "does anything use this". A word set is exact about *absence*: a file
    /// whose set does not hold the word cannot mention it, in code or in a comment or in a
    /// string. So a name no other file's set holds is used by nobody, and one that some set does
    /// hold has only failed to be ruled out -- which is the direction a report about absence has
    /// to be wrong in.
    ///
    /// A scan of the project's own files with a binary search each, which is what makes this
    /// affordable inside a diagnostics budget where a reference sweep is not: 1 674 files at the
    /// median 88 words is a few hundred microseconds, against 2.8 ms for the sweep that would
    /// otherwise answer it.
    pub fn mentioned_outside(&self, file: FileId, name: &str) -> bool {
        self.mentioned_outside_among(file, &[name])[0]
    }

    /// [`Self::mentioned_outside`] for many names at once, in one pass over the file table.
    ///
    /// The table holds every file the project reaches -- 28 886 on the reference checkout, of
    /// which 1 753 carry a word set -- so asking one name at a time walked all of them per name.
    /// A file declaring two hundred methods paid that two hundred times, and it was nine of the
    /// sixteen milliseconds its diagnostics cost.
    ///
    /// Each name is hashed once rather than once per file, which on the same file was the other
    /// half of the cost: three hundred thousand hashes of the same two hundred strings.
    pub fn mentioned_outside_among(&self, file: FileId, names: &[&str]) -> Vec<bool> {
        let needles: Vec<Box<[u64]>> = names
            .iter()
            .map(|name| crate::words::WordSet::needle(name))
            .collect();
        let mut found = vec![false; names.len()];
        let mut left = names.len();
        for (&id, entry) in &self.files {
            // A file with no word set is a dependency or a stub, which is not searched: the set
            // is kept for the project's own code because that is the only code a sweep reads and
            // the only code anybody renames. Treating a missing set as "may hold" instead would
            // make every answer yes, since a real checkout is 27 000 files of dependency.
            let Some(words) = entry.words.as_ref().filter(|_| id != file) else {
                continue;
            };
            for (seen, needle) in found.iter_mut().zip(&needles) {
                if !*seen && words.may_hold_words(needle) {
                    *seen = true;
                    left -= 1;
                }
            }
            if left == 0 {
                break;
            }
        }
        found
    }

    /// Whether anything in the project gives a closure this class's scope.
    ///
    /// The filter in front of a claim that a private member is reachable from its own file only.
    /// By short name, and accumulating rather than withdrawn with the file that wrote it: both
    /// make it suppress more than it must, which is the direction a check about *absence* has to
    /// be wrong in.
    pub fn is_bound_scope(&self, short_name: &str) -> bool {
        self.bound_scopes
            .iter()
            .any(|known| known.as_str().eq_ignore_ascii_case(short_name))
    }

    fn note_receiver_out(&mut self, name: &Name) {
        if !self
            .receiver_out
            .iter()
            .any(|known| known.as_str().eq_ignore_ascii_case(name.as_str()))
        {
            self.receiver_out.push(name.clone());
        }
    }

    /// Whether anything in the project declares a method of this name that says what its receiver
    /// holds once it returns. See [`Self::receiver_out`].
    pub fn declares_receiver_out(&self, name: &str) -> bool {
        self.receiver_out
            .iter()
            .any(|known| known.as_str().eq_ignore_ascii_case(name))
    }

    fn note_never_returning(&mut self, name: &Name) {
        if !self
            .never_returning
            .iter()
            .any(|known| known.as_str().eq_ignore_ascii_case(name.as_str()))
        {
            self.never_returning.push(name.clone());
        }
    }

    fn note_set_hooked(&mut self, name: &Name) {
        if !self
            .set_hooked
            .iter()
            .any(|known| known.as_str() == name.as_str())
        {
            self.set_hooked.push(name.clone());
        }
    }

    /// Whether any class in the project declares a property of this name with a `set` hook.
    ///
    /// The filter in front of reporting a write to a read-only property. Exactly, because PHP
    /// compares property names exactly -- `$name` and `$Name` are two properties.
    pub fn is_set_hooked(&self, name: &str) -> bool {
        self.set_hooked.iter().any(|known| known.as_str() == name)
    }

    /// The folded short name of every class the project declares `#[Attribute]` on.
    ///
    /// Sorted, deduplicated, and a superset: what is offered from it is still checked against the
    /// class's own skeleton. See [`Self::attributes`].
    pub fn attribute_names(&self) -> &[NameKey] {
        &self.attributes
    }

    /// Whether anything in the project declares a callable of this name that returns `never`.
    ///
    /// The filter in front of a full resolution. `false` is the answer for nearly every call in a
    /// project and costs a scan of a handful of short strings.
    pub fn declares_never_returning(&self, name: &str) -> bool {
        self.never_returning
            .iter()
            .any(|known| known.as_str().eq_ignore_ascii_case(name))
    }

    /// Forgets a file entirely, as when it is deleted on disk.
    pub fn remove(&mut self, file: FileId) {
        self.retract(file);
        self.files.remove(&file);
    }

    /// The catalog keys a file's declarations claim, as `(catalog, key, class)`.
    ///
    /// One function, used by both [`Self::claim`] and [`Self::retract`], so that what is withdrawn
    /// is exactly what was recorded. Two derivations of this is how a re-ingested file comes to
    /// leave a key behind pointing at a class it no longer declares.
    ///
    /// A catalog key is not a name PHP has -- it is an entity type id, a plugin id, a service id --
    /// so it is not in the symbol catalog. What makes it findable is that the class carrying the
    /// attribute *is* the answer.
    fn declared_keys(&self, scope: &FileScope) -> Vec<(Box<str>, Box<str>, Box<str>)> {
        if !self.returns.names_any_attribute_key() {
            return Vec::new();
        }

        scope
            .attribute_literals()
            .iter()
            // An entry of an array argument is never the key itself: it is half of a derived one,
            // which `Self::derived_keys` spells from two literals at once.
            .filter(|literal| literal.key.is_none())
            .filter_map(|literal| {
                let catalog = self
                    .returns
                    .catalog_of_attribute(literal.attribute.as_str(), &literal.argument)?;
                let symbol = scope.symbols().get(literal.symbol as usize)?;
                Some((
                    catalog.into(),
                    literal.value.clone(),
                    symbol.fqn.as_str().into(),
                ))
            })
            .collect()
    }

    /// The catalog keys a file's declarations *spell*, as `(catalog, key, detail)`.
    ///
    /// Apart from [`Self::declared_keys`] because these name nothing: `entity.node.canonical` is a
    /// route generated from the `canonical` link template of the `node` entity type, and the class
    /// carrying that template is not what the route resolves to. What travels instead is the entry's
    /// own value -- the path -- which is the line a route read from YAML already shows.
    ///
    /// Grouped by declaration, because a derived key needs two literals of the same occurrence: the
    /// argument that names it and the entry that keys it. One class declares one such attribute, so
    /// the declaration is the occurrence.
    ///
    /// Used by both `claim` and `retract`, for the reason [`Self::declared_keys`] gives.
    fn derived_keys(&self, scope: &FileScope) -> Vec<(Box<str>, Box<str>, Box<str>)> {
        if !self.returns.derives_any_key() {
            return Vec::new();
        }

        let mut found = Vec::new();
        let literals = scope.attribute_literals();
        let mut at = 0;
        while at < literals.len() {
            let symbol = literals[at].symbol;
            let attribute = literals[at].attribute.clone();
            let end = literals[at..]
                .iter()
                .position(|literal| {
                    literal.symbol != symbol || literal.attribute.as_str() != attribute.as_str()
                })
                .map_or(literals.len(), |offset| at + offset);
            let written = literals[at..end]
                .iter()
                .map(|literal| {
                    (
                        literal.argument.clone(),
                        &*literal.value,
                        literal.key.as_deref(),
                    )
                })
                .collect::<Vec<_>>();
            for (catalog, key, detail) in self.returns.derived_keys_of(attribute.as_str(), &written)
            {
                found.push((catalog.into(), key, detail.into()));
            }
            at = end;
        }
        found
    }

    /// The catalog keys a file's *declaration names* carry, as `(catalog, key, detail)`.
    ///
    /// A hook implementation is a plain function whose name is the contract:
    /// `app_core_notifications_alter()` implements `notifications_alter` because it is named
    /// `<extension>_<hook>`, and nothing else in the checkout says the hook exists. So the name is
    /// the only place to read it from.
    ///
    /// Functions only. A class named `<extension>_<something>` is not a convention any framework
    /// here has, and walking every class's name would put the whole vendor tree through the split.
    ///
    /// Used by both `claim` and `retract`, for the reason [`Self::declared_keys`] gives.
    fn derived_names(&self, scope: &FileScope) -> Vec<(Box<str>, Box<str>, Box<str>)> {
        if !self.returns.derives_any_name() {
            return Vec::new();
        }

        scope
            .symbols()
            .iter()
            .filter(|symbol| symbol.kind == SymbolKind::Function)
            .filter_map(|symbol| {
                let name = short_name(symbol.fqn.as_str());
                let (catalog, key) = self.returns.name_key_of(name)?;
                Some((catalog.into(), key, symbol.fqn.as_str().into()))
            })
            .collect()
    }

    /// Every catalog key one file's declarations carry, in all three of the ways they can.
    ///
    /// One reader rather than three loops at each caller, because there are three callers -- ingest
    /// through [`Self::claim`], its mirror in [`Self::retract`], and [`Self::adopt`] taking on a
    /// stored index -- and the three kinds arrived separately. `adopt` re-claimed the *declared*
    /// keys alone, so a warm start held every key an attribute named and none that a declaration
    /// spelled or that a function's own name carried: 1 038 hook names and 277 generated routes
    /// present on a cold start and gone on every start after it, with nothing failing anywhere.
    ///
    /// So a fourth kind must not be addable to one caller and not the others. Added here, it
    /// reaches all three, and the callers below have nothing to say about which kinds exist.
    ///
    /// Returned as a list rather than claimed in place because two of the callers hold a borrow of
    /// the file table while they work; see [`Self::retract`].
    fn catalog_claims(&self, scope: &FileScope) -> Vec<CatalogClaim> {
        let declared = self
            .declared_keys(scope)
            .into_iter()
            .map(|(catalog, key, class)| CatalogClaim {
                catalog,
                key,
                value: ClaimValue::Class(class),
            });
        let spelled = self
            .derived_keys(scope)
            .into_iter()
            .chain(self.derived_names(scope))
            .map(|(catalog, key, detail)| CatalogClaim {
                catalog,
                key,
                value: ClaimValue::Detail(detail),
            });
        declared.chain(spelled).collect()
    }

    /// Records every name a file declares in the catalog.
    ///
    /// One place, because a restored file has to claim exactly what a freshly ingested one does. Two
    /// copies of this loop is how a cached index comes to resolve a name the built one does not.
    fn claim(&mut self, file: FileId, scope: &FileScope) {
        for claim in self.catalog_claims(scope) {
            claim.into_table(&mut self.returns, file);
        }

        for (index, symbol) in scope.symbols().iter().enumerate() {
            let Some(key) = CatalogKey::for_symbol(&mut self.interner, symbol.kind, &symbol.fqn)
            else {
                continue;
            };
            // Only a name somebody could type goes into the short-name index; see
            // `OwnerCatalog::insert`. Derived from the symbol here and from the same field in
            // `retract`, so a withdrawal cannot disagree with what the claim recorded.
            let short = symbol
                .is_nameable()
                .then(|| self.interner.key(short_name(symbol.fqn.as_str())));
            let namespaces = match symbol.is_nameable() {
                true => OwnerCatalog::namespaces_of(&mut self.interner, &symbol.fqn),
                false => Vec::new(),
            };
            self.owners.insert(
                key,
                short,
                Declarer {
                    file,
                    symbol: u32::try_from(index).expect("symbol count fits in u32"),
                    kind: symbol.kind,
                },
                namespaces,
            );
        }
    }

    /// Withdraws every catalog claim a file previously made.
    ///
    /// Derived from the symbols still held for that file rather than from a reverse index: it is the
    /// same information, exactly, and one copy of a fact cannot disagree with itself.
    fn retract(&mut self, file: FileId) {
        let Some(previous) = self.files.get(&file) else {
            return;
        };
        let declared: Vec<Name> = previous
            .classes
            .iter()
            .map(|class| class.fqn.clone())
            .collect();
        let used: Vec<u64> = previous
            .classes
            .iter()
            .flat_map(|class| class.uses.iter())
            .map(|used| Self::trait_key(used.fqn.as_str()))
            .collect();
        self.forget_flattened_from(&declared);
        for key in used {
            if let Some(files) = self.trait_users.get_mut(&key) {
                if let Ok(at) = files.binary_search(&file) {
                    files.remove(at);
                }
                if files.is_empty() {
                    self.trait_users.remove(&key);
                }
            }
        }
        // Collected before the withdrawal because deriving them borrows the file table that the
        // withdrawal needs mutably -- which is also why the reader hands back a list.
        let claimed = self.catalog_claims(&previous.scope);
        for claim in claimed {
            self.returns.withdraw_key(file, &claim.catalog, &claim.key);
        }
        // What the file was seen to reach goes with it, keyed like the claims above: a file that
        // no longer exists cannot be the reason a member is alive.
        self.lock_reached().forget(file);

        // Withdrawn from the rows still held for the file, so what leaves is exactly what was
        // recorded -- the same rule the declared keys above follow, and the reason neither is
        // derived twice.
        for row in self.dispatches.remove(&file).unwrap_or_default() {
            self.returns
                .withdraw_dispatch(file, &row.catalog, row.call.name.as_str());
        }

        // A kind is declared by exactly one class, so the file that declared it is the file that
        // takes it away. Withdrawn from what was recorded rather than by scanning the tables,
        // which is §8's rule about a file's claims being keyed and never scanned.
        self.stated.remove(&file);
        self.renders.remove(&file);
        self.configured_this.remove(&file);
        if self.schemas.remove(&file).is_some() {
            self.schema = std::sync::OnceLock::new();
        }
        self.class_defaults.remove(&file);
        self.constructions.remove(&file);
        self.macros.remove(&file);
        for table in self.declared_tables.remove(&file).unwrap_or_default() {
            self.returns
                .withdraw_property_table(&table.catalog, &table.kind);
        }

        let Some(previous) = self.files.get(&file) else {
            return;
        };
        // Collected first because computing a catalog key interns, and interning needs the mutable
        // borrow this one is holding. The names are `Arc`s, so the clone is a refcount bump.
        let declared = previous
            .scope
            .symbols()
            .iter()
            .map(|symbol| (symbol.kind, symbol.fqn.clone(), symbol.is_nameable()))
            .collect::<Vec<_>>();

        // Enumerated over the same list `claim` enumerated, so each index is the one its claim
        // recorded.
        for (index, (kind, fqn, is_nameable)) in declared.into_iter().enumerate() {
            let Some(key) = CatalogKey::for_symbol(&mut self.interner, kind, &fqn) else {
                continue;
            };
            let short = is_nameable.then(|| self.interner.key(short_name(fqn.as_str())));
            let namespaces = match is_nameable {
                true => OwnerCatalog::namespaces_of(&mut self.interner, &fqn),
                false => Vec::new(),
            };
            let index = u32::try_from(index).expect("symbol count fits in u32");
            self.owners
                .remove_claim(&key, short.as_ref(), (file, index), &namespaces);
        }
    }

    /// The declarers of a name, best first.
    ///
    /// Ranking is by origin then path, never by `FileId`. A file id reflects the order files were
    /// discovered, so ranking by it would make the winner depend on discovery order — and then an
    /// index built by applying edits could disagree with one built from scratch, which is the
    /// invariant everything else is checked against.
    ///
    /// Within one tier, a class the autoloader would load from that file before one it would not:
    /// composer decides which file PHP reads for a class, and a package's IDE stub -- Eloquent Power
    /// Joins ships `.stubs.php` declaring `Illuminate\Database\Eloquent\Builder` with its own six
    /// methods -- sorts before `laravel/framework` by path and shadowed every method of the real
    /// builder: 85 accesses on Laravel.io reaching no member.
    pub fn rank(&self, declarers: &[Declarer]) -> Vec<Declarer> {
        let mut ranked = declarers.to_vec();
        if ranked.len() < 2 {
            return ranked;
        }
        ranked.sort_by_cached_key(|declarer| {
            self.files.get(&declarer.file).map(|skeleton| {
                // By symbol index, which is what the declarer already is: the same identity the
                // catalog stored, so there is nothing to search and nothing to match on.
                let symbol = skeleton.scope.symbols().get(declarer.symbol as usize);
                let demoted = symbol.and_then(|symbol| match symbol {
                    _ if symbol.is_ignored() => Some(IGNORED),
                    _ if symbol.is_conditional() => Some(CONDITIONAL),
                    _ => None,
                });
                let autoloaded = symbol.is_some_and(|symbol| {
                    symbol.kind.is_class_like()
                        && self.autoloads(symbol.fqn.as_str(), &skeleton.path)
                });
                (
                    demoted.unwrap_or_else(|| precedence(skeleton.origin, &skeleton.path)),
                    !autoloaded,
                    // The path relative to the root, so ranking does not depend on where the
                    // project happens to be checked out.
                    self.display_path(skeleton).to_owned(),
                    declarer.symbol,
                )
            })
        });
        ranked
    }

    /// Whether composer's autoloader reads the class-like `fqn` from `path`: a PSR-4 rule maps a
    /// prefix of the name onto a directory, and the rest of the name is the file below it.
    fn autoloads(&self, fqn: &str, path: &Path) -> bool {
        self.namespace_rules.iter().any(|(prefix, directory)| {
            let prefix = prefix.trim_end_matches('\\');
            let rest = match prefix.is_empty() {
                true => Some(fqn),
                false => fqn
                    .strip_prefix(prefix)
                    .and_then(|rest| rest.strip_prefix('\\')),
            };
            rest.is_some_and(|rest| {
                let mut expected = directory.clone();
                for segment in rest.split('\\') {
                    expected.push(segment);
                }
                expected.set_extension("php");
                expected == path
            })
        })
    }

    /// The member chain one declaration hands back, for a caller that resolved a call to it.
    ///
    /// Keyed the way [`Self::declared_throws`] is and for the same reason: resolving a call already
    /// answers with the file and the declaring class.
    pub fn declared_return_path(
        &self,
        file: FileId,
        owner: Option<&str>,
        name: &str,
    ) -> Option<&crate::ReturnedRow> {
        self.files.get(&file)?.returned_paths.iter().find(|row| {
            row.name.as_str().eq_ignore_ascii_case(name)
                && match (&row.owner, owner) {
                    (Some(declared), Some(asked)) => declared.as_str() == asked,
                    (None, None) => true,
                    _ => false,
                }
        })
    }

    /// What one declaration's `@throws` tags named, for a caller that resolved a call to it.
    ///
    /// Keyed by file because that is what resolving a call already answers: a `SymbolLocation`
    /// carries the file and the declaring class, so nothing project-wide has to be kept. A linear
    /// scan of the file's rows, which is right rather than lazy -- the table holds only the
    /// declarations that wrote a tag, so a file has a handful and most have none at all.
    pub fn declared_throws(&self, file: FileId, owner: Option<&str>, name: &str) -> &[Name] {
        let Some(entry) = self.files.get(&file) else {
            return &[];
        };
        entry
            .throws
            .iter()
            .find(|row| {
                row.name.as_str().eq_ignore_ascii_case(name)
                    && match (&row.owner, owner) {
                        // A method's owner is compared exactly: two classes of one project differ
                        // only by namespace often enough that a short-name match would answer
                        // about the wrong one.
                        (Some(declared), Some(asked)) => declared.as_str() == asked,
                        (None, None) => true,
                        _ => false,
                    }
            })
            .map_or(&[][..], |row| &row.classes)
    }

    /// The member chain every declaration in the file hands back, for the ones that name no type.
    ///
    /// Its own pass beside [`Self::collect_throws`], and resolved the same way: the root's class
    /// name against the declaring file's own imports, which nothing else in the project can change.
    /// The *steps* stay names -- what class each walks into is the caller's question, and answering
    /// it here would need every file indexed before this one.
    fn collect_returned_paths(
        interner: &mut Interner,
        scope: &FileScope,
        hir: &FileHir,
    ) -> Box<[crate::ReturnedRow]> {
        let mut found = Vec::new();

        let resolve_root = |interner: &mut Interner,
                            namespace: &phrost_defmap::NamespaceScope,
                            path: &phrost_hir::ReturnedPathData| {
            match &path.root {
                // `$this`, `static::` and `self::` all mean the class the *caller* reached this
                // declaration through, which is a better answer than the one that declared it.
                phrost_hir::ReturnRootData::Receiver => None,
                phrost_hir::ReturnRootData::Named(name_ref) => {
                    let name_ref = hir.name_ref(*name_ref);
                    let resolution = namespace.resolve(
                        name_ref.path.as_str(),
                        name_ref.anchor,
                        ImportKind::ClassLike,
                    );
                    Some(interner.name(resolution.primary().fqn()))
                }
            }
        };

        // A `Foo::class` is a name as written, so it resolves where the root does and for the same
        // reason: the file that wrote it is the only place its imports are known.
        let resolve_key = |interner: &mut Interner,
                           namespace: &phrost_defmap::NamespaceScope,
                           path: &phrost_hir::ReturnedPathData| {
            path.key.as_ref().map(|key| match key {
                phrost_hir::ReturnedKeyData::Text(text) => crate::ReturnedKey::Text(text.clone()),
                phrost_hir::ReturnedKeyData::Class { path, anchor } => {
                    let resolution =
                        namespace.resolve(path.as_str(), *anchor, ImportKind::ClassLike);
                    crate::ReturnedKey::Class(interner.name(resolution.primary().fqn()))
                }
            })
        };

        for (id, class) in hir.classes() {
            let namespace = scope.scope_of(class.namespace);
            let owner = scope
                .symbol_of(DeclarationSource::Class(id))
                .map_or_else(|| class.name.clone(), |symbol| symbol.fqn.clone());
            for &member in hir.member_list(class.members) {
                let data = hir.member(member);
                let Some(path) = data.returned_path.as_ref() else {
                    continue;
                };
                let root = resolve_root(interner, namespace, path);
                let key = resolve_key(interner, namespace, path);
                found.push(crate::ReturnedRow {
                    owner: Some(owner.clone()),
                    name: data.name.clone(),
                    root,
                    steps: path.steps.clone(),
                    key,
                });
            }
        }

        for (id, function) in hir.functions() {
            let Some(path) = function.returned_path.as_ref() else {
                continue;
            };
            let namespace = scope.scope_of(function.namespace);
            let root = resolve_root(interner, namespace, path);
            let key = resolve_key(interner, namespace, path);
            found.push(crate::ReturnedRow {
                owner: None,
                // Fully qualified, because that is what resolving a call to a free function answers
                // with -- the rule `collect_throws` states beside its own.
                name: scope
                    .symbol_of(DeclarationSource::Function(id))
                    .map_or_else(|| function.name.clone(), |symbol| symbol.fqn.clone()),
                root,
                steps: path.steps.clone(),
                key,
            });
        }

        found.into_boxed_slice()
    }

    /// What every declaration in the file documented itself as throwing.
    ///
    /// Its own pass rather than a field filled inside [`Self::collect_classes`], because the
    /// answer is a *name* and that function's business is types: a `@throws` class goes into no
    /// type table, and threading a second output through a function that builds rows would tie the
    /// sparse table's shape to the dense one's. Resolved here for the reason everything else is --
    /// against the declaring file's own namespace and imports, which nothing else in the project
    /// can change.
    ///
    /// Only declarations that wrote a tag appear, which is what keeps the table worth having.
    fn collect_throws(
        interner: &mut Interner,
        scope: &FileScope,
        hir: &FileHir,
    ) -> Box<[crate::ThrownRow]> {
        let mut found = Vec::new();

        let resolve = |interner: &mut Interner,
                       namespace: &phrost_defmap::NamespaceScope,
                       list: lspkit_arena::IdSlice<phrost_hir::TypeRefId>| {
            let classes: Box<[Name]> = hir
                .type_ref_list(list)
                .iter()
                .filter_map(|&id| match hir.type_ref(id) {
                    // A tag naming anything else -- `@throws mixed`, or a union operand that is not
                    // a class -- says nothing about what is thrown and is left out rather than
                    // recorded as a name nothing declares.
                    phrost_hir::TypeRefData::Named(name_ref) => {
                        let name_ref = hir.name_ref(*name_ref);
                        let resolution = namespace.resolve(
                            name_ref.path.as_str(),
                            name_ref.anchor,
                            ImportKind::ClassLike,
                        );
                        Some(interner.name(resolution.primary().fqn()))
                    }
                    _ => None,
                })
                .collect();
            classes
        };

        for (id, class) in hir.classes() {
            let namespace = scope.scope_of(class.namespace);
            let owner = scope
                .symbol_of(DeclarationSource::Class(id))
                .map_or_else(|| class.name.clone(), |symbol| symbol.fqn.clone());
            for &member in hir.member_list(class.members) {
                let data = hir.member(member);
                if data.throws.is_empty() {
                    continue;
                }
                let classes = resolve(interner, namespace, data.throws);
                if classes.is_empty() {
                    continue;
                }
                found.push(crate::ThrownRow {
                    owner: Some(owner.clone()),
                    name: data.name.clone(),
                    classes,
                });
            }
        }

        for (id, function) in hir.functions() {
            if function.throws.is_empty() {
                continue;
            }
            let namespace = scope.scope_of(function.namespace);
            let classes = resolve(interner, namespace, function.throws);
            if classes.is_empty() {
                continue;
            }
            found.push(crate::ThrownRow {
                owner: None,
                // Fully qualified, because that is what resolving a call to a free function
                // answers with -- where a *method* is answered by its own short name and the class
                // that declares it. Keying this by `function.name` instead looked right and made
                // every namespaced function's tag unfindable.
                name: scope
                    .symbol_of(DeclarationSource::Function(id))
                    .map_or_else(|| function.name.clone(), |symbol| symbol.fqn.clone()),
                classes,
            });
        }

        found.into_boxed_slice()
    }

    /// Builds one entry per declared function: its parameters and its return type.
    ///
    /// Driven off the declared symbols rather than off the HIR's function list, because the symbol
    /// index is the identity the catalog stores — deriving it afterwards would mean matching on
    /// something else and hoping the two agree.
    fn collect_functions(
        interner: &mut Interner,
        types: &mut TypeTable,
        scope: &FileScope,
        hir: &FileHir,
    ) -> Vec<FunctionRow> {
        scope
            .symbols()
            .iter()
            .enumerate()
            .filter_map(|(index, symbol)| match symbol.source {
                DeclarationSource::Function(id) => Some((index, id)),
                _ => None,
            })
            .map(|(index, id)| {
                let data = hir.function(id);
                let mut lower = TypeLowerer {
                    hir,
                    namespace: scope.scope_of(data.namespace),
                    interner,
                    types,
                };
                let ty = lower.lower_optional(data.return_type);
                let doc_ty = lower.lower_optional(data.doc_return_type);
                let params = hir
                    .param_list(data.params)
                    .iter()
                    .map(|&id| lower.param(id))
                    .collect();

                let contract = data
                    .contract
                    .as_ref()
                    .and_then(|contract| lower.contract(contract))
                    .map(Box::new);

                FunctionRow {
                    symbol: u32::try_from(index).expect("fewer than 4G symbols in one file"),
                    params,
                    ty,
                    doc_ty,
                    returns_by_reference: data.returns_by_reference,
                    contract,
                    reads_own_arguments: data.reads_own_arguments,
                    is_no_discard: data.is_no_discard,
                    is_deprecated: data.is_deprecated,
                }
            })
            .collect()
    }

    /// Builds one entry per declared class: its resolved relations, its trait adaptations, and its
    /// own members.
    ///
    /// Relations are resolved here rather than on demand because resolution depends only on the
    /// file's own namespace and imports — not on what else exists — so the answer cannot go stale
    /// when another file changes. Class-like names have no global fallback, so there is exactly one
    /// candidate per relation.
    fn collect_classes(
        interner: &mut Interner,
        types: &mut TypeTable,
        scope: &FileScope,
        hir: &FileHir,
    ) -> Vec<ClassSkeleton> {
        hir.classes()
            .map(|(id, class)| {
                let namespace = scope.scope_of(class.namespace);
                let mut resolve_one = |name_ref_id: phrost_hir::NameRefId| {
                    let name_ref = hir.name_ref(name_ref_id);
                    let resolution = namespace.resolve(
                        name_ref.path.as_str(),
                        name_ref.anchor,
                        ImportKind::ClassLike,
                    );
                    ClassRef {
                        fqn: interner.name(resolution.primary().fqn()),
                        range: name_ref.range,
                    }
                };
                let mut resolve = |list| {
                    hir.name_ref_list(list)
                        .iter()
                        .map(|&id| resolve_one(id))
                        .collect::<Vec<_>>()
                };

                let extends = resolve(class.extends);
                let implements = resolve(class.implements);
                let uses = resolve(class.uses);
                let mixins = hir
                    .type_ref_list(class.mixins)
                    .iter()
                    .filter_map(|&id| mixin_name(hir, id))
                    .map(&mut resolve_one)
                    .collect::<Box<[_]>>();

                let adaptations = hir
                    .adaptation_list(class.trait_adaptations)
                    .iter()
                    .map(|&id| {
                        let data = hir.trait_adaptation(id);
                        TraitAdaptation {
                            trait_fqn: data.trait_name.map(|name| resolve_one(name).fqn),
                            method: data.method.clone(),
                            method_range: data.method_range,
                            kind: match &data.kind {
                                phrost_hir::TraitAdaptationKind::Precedence { excluded } => {
                                    TraitAdaptationKind::Precedence {
                                        excluded: hir
                                            .name_ref_list(*excluded)
                                            .iter()
                                            .map(|&id| resolve_one(id).fqn)
                                            .collect(),
                                    }
                                }
                                phrost_hir::TraitAdaptationKind::Alias {
                                    alias,
                                    visibility,
                                    ..
                                } => TraitAdaptationKind::Alias {
                                    alias: alias.clone(),
                                    visibility: *visibility,
                                },
                            },
                        }
                    })
                    .collect();

                let mut lower = TypeLowerer {
                    hir,
                    namespace,
                    interner,
                    types,
                };
                let members = hir
                    .member_list(class.members)
                    .iter()
                    .map(|&id| {
                        let data = hir.member(id);
                        MemberRow {
                            // Interned rather than cloned, because the HIR may have been lowered
                            // against another interner -- see `ingest_lowered`. A cloned `Arc` reads
                            // identically and is a second allocation of a string the project has
                            // hundreds of thousands of.
                            name: lower.interner.name(data.name.as_str()),
                            reads_own_arguments: data.reads_own_arguments,
                            kind: match data.kind {
                                phrost_hir::MemberKind::Method => MemberKind::Method,
                                phrost_hir::MemberKind::Property => MemberKind::Property,
                                phrost_hir::MemberKind::ClassConstant => MemberKind::ClassConstant,
                                phrost_hir::MemberKind::EnumCase => MemberKind::EnumCase,
                            },
                            visibility: data.visibility,
                            set_visibility: data.set_visibility,
                            doc_ty_from_body: data.type_from_body,
                            doc_ty_refused: data.doc_type_refused,
                            modifiers: member_modifiers(data.modifiers),
                            // Resolved once, at ingest, into the project's shared type table.
                            // Resolution depends only on this file's imports, so it cannot go stale;
                            // and because the table interns, a few thousand distinct types cover a
                            // project's hundreds of thousands of members.
                            ty: lower.lower_optional(data.type_ref),
                            doc_ty: lower.lower_optional(data.doc_type_ref),
                            params: hir
                                .param_list(data.params)
                                .iter()
                                .map(|&id| lower.param(id))
                                .collect(),
                            name_range: data.name_range,
                            full_range: data.full_range,
                            docblock_range: data.docblock.map(|id| hir.docblock(id).range),
                            contract: data
                                .contract
                                .as_ref()
                                .and_then(|contract| lower.contract(contract))
                                .map(Box::new),
                            from_docblock: data.from_docblock,
                            ty_from_value: data.ty_from_value,
                        }
                    })
                    .collect();

                ClassSkeleton {
                    fqn: scope
                        .symbol_of(DeclarationSource::Class(id))
                        .map_or_else(|| class.name.clone(), |symbol| symbol.fqn.clone()),
                    kind: match class.kind {
                        phrost_hir::ClassKind::Class => ClassKind::Class,
                        phrost_hir::ClassKind::Interface => ClassKind::Interface,
                        phrost_hir::ClassKind::Trait => ClassKind::Trait,
                        phrost_hir::ClassKind::Enum => ClassKind::Enum,
                    },
                    is_abstract: class.modifiers.is_abstract,
                    enum_backing: lower.lower_optional(class.enum_backing),
                    is_final: class.modifiers.is_final,
                    is_readonly: class.modifiers.is_readonly,
                    is_attribute: class.modifiers.is_attribute,
                    is_deprecated: class.modifiers.is_deprecated,
                    extends,
                    implements,
                    uses,
                    adaptations,
                    members,
                    templates: hir
                        .template_list(class.templates)
                        .iter()
                        .map(|&id| {
                            let data = hir.template(id);
                            TemplateRow {
                                name: lower.interner.name(data.name.as_str()),
                                bound: lower.lower_optional(data.bound),
                            }
                        })
                        .collect(),
                    supertype_arguments: hir
                        .type_ref_list(class.supertype_arguments)
                        .iter()
                        .chain(
                            // A `@mixin Collection<Item>` is both: a supertype for lookup and an
                            // argument for one. Only the parameterised ones -- a bare `@mixin Other`
                            // has nothing to supply.
                            hir.type_ref_list(class.mixins).iter().filter(|&&id| {
                                matches!(hir.type_ref(id), phrost_hir::TypeRefData::Generic { .. })
                            }),
                        )
                        .map(|&id| lower.lower(id))
                        .collect(),
                    mixins,
                }
            })
            .collect()
    }
}

/// Looks a class up by fully-qualified name, so flattening can walk a hierarchy across files.
/// One catalog key a file's declarations carry, on its way into or out of the contract table.
///
/// A type rather than a tuple because the two kinds are claimed by different calls and only the
/// claiming direction cares which: see [`ProjectSkeleton::catalog_claims`], whose whole purpose is
/// that a caller need not know how many kinds there are.
struct CatalogClaim {
    catalog: Box<str>,
    key: Box<str>,
    value: ClaimValue,
}

/// What a claim says the key is, which is the one thing the two kinds do not share.
enum ClaimValue {
    /// The class that declares the key, for a catalog whose keys name one.
    Class(Box<str>),
    /// A line to show beside a key that names nothing -- the path a link template writes, or the
    /// function whose name carried the key. Answering a class here would make an argument bound to
    /// the catalog resolve to it; see `ReturnContracts::claim_derived_key`.
    Detail(Box<str>),
}

impl CatalogClaim {
    /// Records the claim, by whichever of the two calls its kind needs.
    fn into_table(self, returns: &mut ReturnContracts, file: FileId) {
        match self.value {
            ClaimValue::Class(class) => {
                returns.claim_key(file, &self.catalog, &self.key, &class);
            }
            ClaimValue::Detail(detail) => {
                returns.claim_derived_key(file, &self.catalog, &self.key, &detail);
            }
        }
    }
}

impl ClassSource for ProjectSkeleton {
    fn class(&self, fqn: &str) -> Option<ClassFacts<'_>> {
        let declarers = self.class_declarers(fqn);
        // The best-ranked declarer, for the same reason every other lookup uses it: a name declared
        // twice has one answer, and it must not depend on discovery order.
        let winner = *self.rank(declarers).first()?;
        let file = self.files.get(&winner.file)?;
        let symbol = file.scope.symbols().get(winner.symbol as usize)?;
        let class = file.classes.iter().find(|class| class.fqn == symbol.fqn)?;

        Some(ClassFacts {
            fqn: &class.fqn,
            kind: class.kind,
            is_abstract: class.is_abstract,
            enum_backing: class.enum_backing,
            extends: &class.extends,
            implements: &class.implements,
            uses: &class.uses,
            adaptations: &class.adaptations,
            mixins: &class.mixins,
            implied_mixins: self.returns.implied_mixins_of(class.fqn.as_str()).0,
            opened: self.returns.opened_methods_of(class.fqn.as_str()),
            tagged_from: self.returns.tag_sources_of(class.fqn.as_str()),
            members: &class.members,
        })
    }
}

/// The memo the flattener reads and fills -- see the `flattened` field.
impl FlattenMemo for ProjectSkeleton {
    /// A class's flattened member table, if a request already built it and nothing it was built
    /// from has changed since. `key` is the folded name, as `IndexView::members` keys it.
    fn flattened(&self, key: &str) -> Option<Arc<MemberTable>> {
        let mut memo = self.lock_flattened();
        memo.tick += 1;
        let tick = memo.tick;
        let (table, used) = memo.tables.get_mut(key)?;
        *used = tick;
        Some(Arc::clone(table))
    }

    /// Keeps a table for the next request.
    ///
    /// Bounded, because a sweep over a whole project would otherwise hold a table for every class
    /// it met, and a Drupal entity's runs to hundreds of members. Past the limit the table asked
    /// for longest ago goes, not the whole memo: a sweep meets a new class in nearly every file and
    /// the base classes under all of them are what is worth keeping. An incomplete table is not
    /// kept at all -- it was cut short by a class the index did not have, and that class arriving
    /// later would not appear among the contributors, so nothing would evict the stale answer.
    fn remember_flattened(&self, key: Box<str>, table: &Arc<MemberTable>) {
        if table.is_incomplete() {
            return;
        }
        let mut memo = self.lock_flattened();
        if memo.tables.len() >= FLATTENED_LIMIT
            && let Some(oldest) = memo
                .tables
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone())
        {
            memo.tables.remove(&oldest);
        }
        memo.tick += 1;
        let tick = memo.tick;
        memo.tables.insert(key, (Arc::clone(table), tick));
    }
}

/// The flattened member tables kept between requests, and a clock for telling which was asked
/// for least recently. See [`ProjectSkeleton::remember_flattened`].
#[derive(Debug, Default)]
struct FlattenedMemo {
    tables: FxHashMap<Box<str>, (Arc<MemberTable>, u64)>,
    tick: u64,
}

/// Whether `path` is inside the directory `root`, compared as bytes.
///
/// `Path::starts_with` is component-wise, and a component walk over 849 autoload roots for every
/// file checked was 15 % of a diagnostics pass -- two checks ask this of every file, and both were
/// spending their whole time in it. Both paths here are absolute and spelled the same way, by the
/// same code, so a byte prefix with a separator after it answers the same question.
fn lies_under(path: &Path, root: &Path) -> bool {
    let (path, root) = (
        path.as_os_str().as_encoded_bytes(),
        root.as_os_str().as_encoded_bytes(),
    );
    path.len() > root.len()
        && path.starts_with(root)
        && (std::path::is_separator(path[root.len()] as char)
            || root
                .last()
                .is_some_and(|last| std::path::is_separator(*last as char)))
}

/// How far a discovery rule follows a hierarchy down.
///
/// A base, the abstract one under it and the class somebody wrote is three; the rest is a guard
/// against a cycle rather than a depth anybody reaches.
const DISCOVERY_DEPTH: usize = 8;

/// One file's contribution to the skeleton.
#[derive(Debug)]
pub struct FileSkeleton {
    origin: SourceOrigin,
    stamp: FileStamp,
    /// What the file's metadata said when these rows were read from disk, so a later start can
    /// keep them without reading it again -- see `FileFingerprint`. `None` for rows built from an
    /// editor's buffer, whose text is not what the disk holds: those are hashed next time.
    fingerprint: Option<FileFingerprint>,
    /// Where the file is, absolutely. Reading it needs this; a relative path would not do.
    path: Arc<Path>,
    scope: FileScope,
    classes: Vec<ClassSkeleton>,
    functions: Vec<FunctionRow>,
    /// Every identifier-shaped word the file's text holds, for the project's own files only.
    ///
    /// Held for the reason `strict_types` below is: deriving it means re-reading the file, which
    /// for a closed file is the fallback §1.2 forbids -- and a reference sweep asks it of every
    /// file the project owns, per request. `None` for a dependency and for a stub, which no sweep
    /// reads, and which is also what makes this 1.6 MB rather than most of the index. See
    /// [`WordSet`](crate::WordSet).
    words: Option<crate::words::WordSet>,
    /// Classes this file names as a `Closure::bind` scope, by short name as written.
    ///
    /// Stored rather than derived, for the reason the word set beside it is: it is read out of a
    /// *body*, and a warm start has none. A handful of names on a project that uses the pattern
    /// and empty on one that does not.
    bound_scopes: Box<[Name]>,
    /// Whether the file declared `declare(strict_types=1)`.
    ///
    /// One bit, held here rather than derived on demand, because deriving it means re-reading and
    /// re-parsing the file -- which for a closed file is the fallback §1.2 forbids. It is a
    /// property of the file a *call* is written in, so every file needs it, not only the open one.
    strict_types: bool,
    /// What this file's declarations documented themselves as throwing. See [`ThrownRow`].
    ///
    /// Stored for the reason the two fields above are: it is read out of a docblock, and the index
    /// keeps ranges rather than text, so a *caller* asking what a callee throws would be a disk
    /// read per call site. Empty for almost every file.
    throws: Box<[crate::ThrownRow]>,
    /// The member chains this file's declarations hand back, for the ones that name no type at
    /// all. See [`ReturnedRow`](crate::ReturnedRow).
    returned_paths: Box<[crate::ReturnedRow]>,
    /// What this file's constants' values state. See [`ConstantRow`](crate::constant::ConstantRow).
    constants: Box<[crate::constant::ConstantRow]>,
}

impl FileSkeleton {
    /// Rebuilds one from a cache.
    #[expect(
        clippy::too_many_arguments,
        reason = "one argument per stored field, by design"
    )]
    pub(crate) fn from_parts(
        origin: SourceOrigin,
        stamp: FileStamp,
        fingerprint: Option<FileFingerprint>,
        path: Arc<Path>,
        scope: FileScope,
        classes: Vec<ClassSkeleton>,
        functions: Vec<FunctionRow>,
        words: Option<crate::words::WordSet>,
        bound_scopes: Box<[Name]>,
        strict_types: bool,
        throws: Box<[crate::ThrownRow]>,
        returned_paths: Box<[crate::ReturnedRow]>,
        constants: Box<[crate::constant::ConstantRow]>,
    ) -> Self {
        Self {
            origin,
            stamp,
            fingerprint,
            path,
            scope,
            classes,
            functions,
            words,
            bound_scopes,
            strict_types,
            throws,
            returned_paths,
            constants,
        }
    }

    /// What this file's constants' values state.
    pub fn constants(&self) -> &[crate::constant::ConstantRow] {
        &self.constants
    }

    /// The stated type of the constant a declared symbol refers to.
    pub fn constant_of(&self, symbol: u32) -> Option<&crate::constant::ConstantRow> {
        self.constants.iter().find(|row| row.symbol == symbol)
    }

    /// What this file's declarations documented themselves as throwing.
    pub fn throws(&self) -> &[crate::ThrownRow] {
        &self.throws
    }

    /// The member chains this file's declarations hand back.
    pub fn returned_paths(&self) -> &[crate::ReturnedRow] {
        &self.returned_paths
    }

    /// The classes this file binds a closure to, by short name.
    pub fn bound_scopes(&self) -> &[Name] {
        &self.bound_scopes
    }

    /// The words the file's text holds, for a project-wide search that must not read it.
    ///
    /// `None` for a file no sweep reads -- a dependency, a stub -- and for one whose rows came
    /// from an editor's buffer before this was recorded. A caller with `None` has learned nothing
    /// and has to read the file, which is what it did for every file before this existed.
    pub fn words(&self) -> Option<&crate::words::WordSet> {
        self.words.as_ref()
    }

    pub fn origin(&self) -> SourceOrigin {
        self.origin
    }

    pub fn stamp(&self) -> FileStamp {
        self.stamp
    }

    pub fn fingerprint(&self) -> Option<FileFingerprint> {
        self.fingerprint
    }

    /// The file's location on disk.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn scope(&self) -> &FileScope {
        &self.scope
    }

    pub fn classes(&self) -> &[ClassSkeleton] {
        &self.classes
    }

    pub fn functions(&self) -> &[FunctionRow] {
        &self.functions
    }

    /// Whether calls written in this file are checked under `strict_types=1`.
    pub fn strict_types(&self) -> bool {
        self.strict_types
    }

    /// The signature of the function a declared symbol refers to.
    ///
    /// Keyed by symbol index because that is what the catalog hands out; a file declares few
    /// functions, so a scan beats a second map from the same key.
    pub fn function_of(&self, symbol: u32) -> Option<&FunctionRow> {
        self.functions
            .iter()
            .find(|function| function.symbol() == symbol)
    }
}

/// One declared class, with everything the semantic layer needs to flatten it.
#[derive(Debug)]
pub struct ClassSkeleton {
    fqn: Name,
    kind: ClassKind,
    is_abstract: bool,
    /// `enum Suit: string` -- the backing type, and `None` for a pure enum or anything else.
    enum_backing: Option<phrost_ty::TypeId>,
    is_final: bool,
    is_readonly: bool,
    /// Whether the declaration carries `#[Attribute]`. See [`FileHir`]'s `ClassModifiers`.
    is_attribute: bool,
    /// Whether `@deprecated` or `#[\Deprecated]` says the class should not be reached for.
    is_deprecated: bool,
    extends: Vec<ClassRef>,
    implements: Vec<ClassRef>,
    uses: Vec<ClassRef>,
    adaptations: Vec<TraitAdaptation>,
    members: Vec<MemberRow>,
    templates: Vec<TemplateRow>,
    /// `@extends Bar<Baz>` and its siblings, each kept as the whole type the tag wrote.
    ///
    /// The type carries both halves already: its base names the supertype, resolved like any other
    /// class reference, and its arguments are what this declaration supplied. Storing it whole means
    /// no second kind of row and no second resolution.
    ///
    /// A boxed slice rather than a `Vec`, because it is empty for all but a handful of classes in a
    /// project and an empty box costs no allocation.
    supertype_arguments: Box<[phrost_ty::TypeId]>,
    /// `@mixin Other` -- classes whose members are reachable through this one.
    ///
    /// Resolved to references like any supertype, because that is what they are for member lookup:
    /// the tag is the only thing in the source that says so, and goto on an inherited member has to
    /// land in the class that declared it.
    mixins: Box<[ClassRef]>,
}

/// A call whose string argument names a key a sibling method writes into an array it builds.
///
/// Drupal's `$form_state->getValue('email')` beside the `$form['email'] = [...]` its form's
/// `buildForm()` wrote.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BuiltKey {
    /// The class or interface declaring the reading call.
    pub class: Box<str>,
    /// The reading call, whose first argument is the key.
    pub method: Box<str>,
    /// What a property key begins with; an array holding one is an element.
    pub property: Box<str>,
    /// The property whose literal value says what an element is.
    pub described_by: Box<str>,
    /// The property filing an element's value under a path whose first entry is its key.
    pub filed_under: Box<str>,
}

/// A call on a test double whose string argument names a method of what the double stands in for.
///
/// PHPUnit configures a mock's subject by name -- `$repo->method('find')`, or
/// `$repo->expects($this->once())->method('find')` one call along -- and the double's own type says
/// what the subject is: `createMock(Repo::class)` is a `MockObject&Repo`. So the string is a method
/// of the very value the call is made on, looked up as `->find()` would be.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StandInMethod {
    /// What makes a value a double: a type one of its operands descends from.
    pub stand_in: Box<str>,
    /// The configuring call.
    pub method: Box<str>,
    /// Which argument names the method.
    pub at: u16,
    /// Calls between the double and the configuring one that hand back a builder for it.
    pub through: Box<[Box<str>]>,
}

/// One `@template T of Bound`, resolved.
///
/// The bound rather than any argument: substituting an argument down a chain of `@extends` is a
/// separate job, and a bound is what the declaration promises about every argument it could take --
/// so reading it offers fewer members than the real type has and never more.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TemplateRow {
    pub name: Name,
    pub bound: Option<phrost_ty::TypeId>,
}

impl ClassSkeleton {
    /// Rebuilds one from a cache. The fields are private, and the codec is a sibling module.
    #[expect(
        clippy::too_many_arguments,
        reason = "one argument per stored field, by design"
    )]
    pub(crate) fn from_parts(
        fqn: Name,
        kind: ClassKind,
        is_abstract: bool,
        enum_backing: Option<phrost_ty::TypeId>,
        is_final: bool,
        is_readonly: bool,
        is_attribute: bool,
        is_deprecated: bool,
        extends: Vec<ClassRef>,
        implements: Vec<ClassRef>,
        uses: Vec<ClassRef>,
        adaptations: Vec<TraitAdaptation>,
        members: Vec<MemberRow>,
        templates: Vec<TemplateRow>,
        supertype_arguments: Box<[phrost_ty::TypeId]>,
        mixins: Box<[ClassRef]>,
    ) -> Self {
        Self {
            fqn,
            kind,
            is_abstract,
            enum_backing,
            is_final,
            is_readonly,
            is_attribute,
            is_deprecated,
            extends,
            implements,
            uses,
            adaptations,
            members,
            templates,
            supertype_arguments,
            mixins,
        }
    }

    /// The `@template` parameters this class-like declares, in the order written.
    pub fn templates(&self) -> &[TemplateRow] {
        &self.templates
    }

    pub fn supertype_arguments(&self) -> &[phrost_ty::TypeId] {
        &self.supertype_arguments
    }

    pub fn mixins(&self) -> &[ClassRef] {
        &self.mixins
    }

    /// Whether the declaration is `abstract`, and so allowed to leave a requirement unmet.
    pub fn is_abstract(&self) -> bool {
        self.is_abstract
    }

    pub(crate) fn enum_backing(&self) -> Option<phrost_ty::TypeId> {
        self.enum_backing
    }

    /// Whether this is an enum with a backing type, `enum Suit: string`.
    pub fn is_backed_enum(&self) -> bool {
        self.enum_backing.is_some()
    }

    pub fn is_final(&self) -> bool {
        self.is_final
    }

    pub fn is_readonly(&self) -> bool {
        self.is_readonly
    }

    /// Whether the declaration carries `#[Attribute]`, which is what makes a class usable as one.
    pub fn is_attribute(&self) -> bool {
        self.is_attribute
    }

    /// Whether the declaration says it is deprecated, by either spelling.
    pub fn is_deprecated(&self) -> bool {
        self.is_deprecated
    }

    /// The `use` clause's `insteadof` and `as` lines, resolved.
    pub fn adaptations(&self) -> &[TraitAdaptation] {
        &self.adaptations
    }

    pub fn fqn(&self) -> &Name {
        &self.fqn
    }

    pub fn kind(&self) -> ClassKind {
        self.kind
    }

    /// Declaration modifiers, in the order PHP writes them.
    ///
    /// Returned as written rather than as booleans a caller has to reassemble: everything that
    /// wants them wants them for display.
    pub fn modifier_words(&self) -> Vec<&'static str> {
        [
            (self.is_abstract, "abstract"),
            (self.is_final, "final"),
            (self.is_readonly, "readonly"),
        ]
        .into_iter()
        .filter_map(|(present, word)| present.then_some(word))
        .collect()
    }

    /// Whether this class directly names `fqn` as a supertype or a trait.
    ///
    /// Direct only, and the resolved name: `extends Repo` after `use App\Repo;` names `App\Repo`,
    /// and a `Repo` two namespaces over is a different class.
    pub fn inherits_from(&self, fqn: &str) -> bool {
        [&self.extends, &self.implements, &self.uses]
            .into_iter()
            .flatten()
            .any(|named| named.fqn.as_str().eq_ignore_ascii_case(fqn))
    }

    pub fn extends(&self) -> &[ClassRef] {
        &self.extends
    }

    pub fn implements(&self) -> &[ClassRef] {
        &self.implements
    }

    pub fn uses(&self) -> &[ClassRef] {
        &self.uses
    }

    pub fn members(&self) -> &[MemberRow] {
        &self.members
    }
}

/// User code wins over vendor, which wins over stubs.
///
/// A project can legitimately declare a name that a dependency also declares — a polyfill, or a
/// forked package — and the copy the developer is editing is the one they mean.
/// Which declaration of a name wins, when several exist.
///
/// A dependency's *test data* ranks below everything, stubs included, and that is the whole reason
/// this is not a match on the origin alone. PHP_CodeSniffer ships its lint fixtures as real files
/// -- `src/Standards/Generic/Tests/PHP/ForbiddenFunctionsUnitTest.inc` declares a bare
/// `function count() {}` -- and under plain `User < Vendor < Stub` that beat the real
/// two-parameter stub. Goto on `count()` landed in a lint fixture and hover said it took no
/// arguments, in every project with PHP_CodeSniffer as a dev dependency. There are 454 such
/// declarations in the one measured here, `sizeof`, `define`, `exit` and `match` among them.
///
/// Ranked rather than skipped, deliberately: the file is still indexed and still answers when
/// nothing else declares the name, so a dependency whose only declaration of something is in a test
/// directory does not vanish. What it cannot do is shadow.
///
/// This answers for the *file*, and [`ProjectSkeleton::rank`] has one tier below every value it
/// returns -- see [`CONDITIONAL`]. Two levels rather than one argument, because only ranking has a
/// declarer to ask: [`ProjectSkeleton::implementers_of`] sorts classes, which carry no symbol index,
/// and a `false` passed there would read as a claim rather than as the absence of one.
fn precedence(origin: SourceOrigin, path: &Path) -> u8 {
    match origin {
        SourceOrigin::User => 0,
        SourceOrigin::Vendor if !is_test_data(path) => 1,
        SourceOrigin::Stub => 2,
        SourceOrigin::Vendor => 3,
    }
}

/// The tier of a declaration PHP reaches only when the code around it runs -- below all four above,
/// whatever file it is in and whoever wrote it.
///
/// `if (!function_exists('str_starts_with')) { function str_starts_with(?string $haystack, ?string
/// $needle) {} }` states its own precedence: it applies when nothing else declared the name, and
/// never otherwise. 402 names on one measured checkout are written that way and 113 of them shadow
/// a bundled stub, so ranking them by origin replaces PHP's own signatures with a polyfill's -- and
/// a polyfill widens its parameters to `?string`, which quietly closes every nullable-argument
/// finding against them.
///
/// Last and not skipped, for the reason a dependency's test data is: the declaration is still the
/// answer when nothing else declares the name, which is the whole case a polyfill exists for.
const CONDITIONAL: u8 = 4;

/// The tier of a declaration its author marked `@ignore` -- below even a conditional one.
///
/// phpDocumentor's "not part of the API", and where a name has two declarers it is the only thing in
/// the source that tells them apart. WordPress's `wp-admin/includes/noop.php` declares nineteen of
/// its commonest functions as `function __() {}` for one minimal load path, beside the real
/// `__( $text, $domain = 'default' )` in `wp-includes/l10n.php`; both are unconditional user code in
/// the same checkout, and the two files are never loaded together, which nothing states. Eighteen of
/// the nineteen carry the tag.
///
/// Believing the noop cost **18 906 of 23 092 findings** on a clean WordPress checkout -- 82 % of
/// everything the checker said, from nineteen declarations, and every one of them the shape
/// `__() takes at most 0 arguments, 1 given`.
///
/// Below `CONDITIONAL` because the claim is weaker still: a polyfill asks to be used when nothing
/// else declared the name, and this asks not to be read as the declaration at all. Last and not
/// skipped, for the same reason: with one declarer it is still the answer.
const IGNORED: u8 = 5;

/// Whether a path runs through a directory a package keeps its tests in.
///
/// A whole path component, so `src/Contest/` and `tests.php` are not caught. Only asked of a
/// dependency: a project's own tests are code the person is editing, and rank first like the rest
/// of it.
fn is_test_data(path: &Path) -> bool {
    path.components().any(|part| {
        matches!(
            part.as_os_str().to_str(),
            Some("tests" | "Tests" | "test" | "Test" | "fixtures" | "Fixtures")
        )
    })
}

/// The class a `@mixin` tag names, whether or not it was given arguments.
fn mixin_name(hir: &FileHir, id: phrost_hir::TypeRefId) -> Option<phrost_hir::NameRefId> {
    match hir.type_ref(id) {
        phrost_hir::TypeRefData::Named(name) => Some(*name),
        phrost_hir::TypeRefData::Generic { base, .. } => mixin_name(hir, *base),
        _ => None,
    }
}

/// A member's declaration words, carried across the boundary one word at a time.
///
/// Named rather than by handing the packed byte over: the two structs are one vocabulary written
/// twice, and passing the bits would make their bit *order* load-bearing and silent -- the shape §8
/// calls two constructors of one rule.
fn member_modifiers(data: phrost_hir::MemberModifiers) -> MemberModifiers {
    let mut out = MemberModifiers::default();
    out.set_static(data.is_static());
    out.set_abstract(data.is_abstract());
    out.set_final(data.is_final());
    out.set_readonly(data.is_readonly());
    out.set_no_discard(data.is_no_discard());
    out.set_override(data.is_override());
    out.set_deprecated(data.is_deprecated());
    out.set_marked(data.is_marked());
    out.hooks = match data.hooks {
        phrost_hir::Hooks::None => phrost_sema::Hooks::None,
        phrost_hir::Hooks::Settable => phrost_sema::Hooks::Settable,
        phrost_hir::Hooks::BackedGet => phrost_sema::Hooks::BackedGet,
        phrost_hir::Hooks::DeclaredGet => phrost_sema::Hooks::DeclaredGet,
        phrost_hir::Hooks::VirtualGet => phrost_sema::Hooks::VirtualGet,
    };
    out
}
