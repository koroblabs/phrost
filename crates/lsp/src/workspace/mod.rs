mod candidates;
pub(crate) mod composer;
mod contributions;
mod templates;

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use phrost_analysis::{ReferenceTarget, SweepQuery};
use phrost_index::{OpenSet, ProjectSkeleton, WordNeedles};
use phrost_model::{FileId, Interner, LineIndex, SourceOrigin};
use phrost_project::{
    AutoloadMap, ComposerJson, PHP_EXTENSIONS, ProjectLayout, discover_php_files,
};
use phrost_stubs::StubSet;
use phrost_syntax_features::format::FormatStyle;
use phrost_vfs::{DocumentVersion, FileStamp, SourceText, Vfs};
use phrost_view::IndexView;

/// How many projects' indexes are kept on disk.
///
/// A cache entry is a megabyte or so and buys a couple of seconds on the next start, so keeping a
/// handful of the projects somebody actually works in is the whole of the benefit. Past that it is
/// storage for roots that will not be opened again.
const CACHED_PROJECTS: usize = 8;

/// The most threads a reference sweep reads on.
///
/// Higher than indexing's four because this holds nothing: a worker here reads a file, answers one
/// question about its bytes and drops them, where a lowering worker keeps an arena and a HIR. The
/// cost that caps indexing is the allocator holding each thread's high-water mark, and there is
/// none of it to hold.
const SCAN_WORKERS: usize = 8;

/// Everything the server knows about one project.
///
/// Held behind a single lock. That is not the end state — background indexing wants a finer grain,
/// and the previous attempt's single lock around everything was a real source of stalls — but a
/// coarse lock that is provably correct is the right thing to have before there is anything to
/// measure. Bootstrap deliberately takes it in short bursts rather than for the whole scan.
#[derive(Debug, Default)]
pub struct Workspace {
    layout: Option<ProjectLayout>,
    /// Open buffers of a language this server does not index but does read: a framework's own
    /// configuration files. Text only, keyed by path, and never registered in the VFS -- there is no
    /// `FileId` for a file the index holds nothing about.
    configs: std::collections::BTreeMap<PathBuf, ConfigBuffer>,
    /// Open templates as the editor holds them, keyed by path: the index holds the PHP each one's
    /// dialect made, and what pairs in a template's structure is read from what was written.
    written_templates: std::collections::BTreeMap<PathBuf, Arc<str>>,
    /// Files the editor does not hold, lowered in the open set because they render a template it
    /// does -- see `templates`.
    beside: std::collections::BTreeSet<FileId>,
    autoload: AutoloadMap,
    /// Whether composer calls this package a `project` rather than something with consumers.
    ///
    /// Read once with the rest of the manifest, and asked by one thing: a report of what nothing
    /// calls is an answer for an application and a list of its public API for a library.
    is_an_application: bool,
    /// Ids of the frameworks detected in this project, for logging.
    frameworks: Vec<&'static str>,
    /// The house style to reprint a file in, when a framework named one.
    ///
    /// Held rather than asked for per request, because it is decided once by which frameworks the
    /// project has -- and asking again would mean re-probing the filesystem to format a keystroke.
    format_style: FormatStyle,
    /// The oldest PHP the project says it runs on -- composer's `require.php` -- which decides what
    /// the formatter may write as well as how it lays it out. See `FormatTarget`.
    php_floor: Option<phrost_model::PhpVersion>,
    /// The rules a reader is shown without asking, and the rules the project named either way.
    ///
    /// Built once here, from the default below and then whatever `phrost.toml` and the client's
    /// `initializationOptions` say -- see [`ProjectConfig`](phrost_project::ProjectConfig) for why
    /// there is only one place that builds it. A second place that read either source would disagree
    /// with this one, and the disagreement shows up as a rule that is off in the editor and on in the
    /// checker.
    lint: LintRules,
    /// Which files are checked and reported -- see [`Diagnose`].
    diagnose: Diagnose,
    /// What the client sent as `initializationOptions`, held verbatim until a root is fixed.
    ///
    /// Held rather than merged on arrival because it is the *last* word over a file that can only be
    /// read once there is a root to read it from -- and because the root is read again when composer
    /// changes, at which point the editor has sent nothing new and this is still what it said.
    editor_config: Option<serde_json::Value>,
    /// Why a configuration source was ignored, one reason per source that was not understood.
    config_errors: Vec<String>,
    /// Extensions beyond PHP's own that a framework said hold PHP.
    ///
    /// Only the contributed ones. PHP's are a constant, so [`Self::php_extensions`] can never come
    /// back empty -- which matters because "is this file PHP" now decides whether a buffer is read
    /// at all, and a server started with no root would otherwise answer no to everything.
    contributed_extensions: Vec<String>,
    /// Extensions a framework said it reads its own answers out of -- Drupal's `.yml`.
    ///
    /// Apart from the PHP ones because the two are asked opposite questions: a file with a *PHP*
    /// extension is indexed for what it declares, and a file with one of these is a reason to read
    /// the project again, because what the providers contributed was read out of it. Empty for a
    /// project with no framework, which is what keeps a plain project from watching every YAML it
    /// happens to contain.
    contributed_config_extensions: Vec<String>,
    /// Code the project reads and does not own -- see `FrameworkProject::dependency_paths`.
    dependency_paths: Vec<PathBuf>,
    /// Files a framework said hold PHP that no autoload rule names.
    ///
    /// Held for the reason the extensions are: finding them means walking the project, and a
    /// re-scan must not pay for that walk again. Paths rather than a rule, because there is no
    /// namespace to map -- these are the files a framework loads by hand.
    framework_sources: Vec<PathBuf>,
    /// Which paths are not this project's code, and which are its code whatever else says.
    ///
    /// Three sources of the first: a framework says what it generates -- a template cache, a
    /// settings file -- the project's `phrost.toml` adds to it, and `PathRules` holds the built-in
    /// names beside them. The second is the project's `include`, which outranks all of it.
    ///
    /// One value rather than two lists, because the rule is asked in two places that have to
    /// agree: discovery steps over an excluded directory and this refuses a buffer opened inside
    /// one. See `phrost_project::paths::PathRules`.
    path_rules: phrost_project::paths::PathRules,
    /// Ids the project switched on itself, so a log can say which were not found by looking.
    configured_frameworks: Vec<&'static str>,
    /// Files the watcher brought in since discovery last ran.
    ///
    /// [`Self::forget_undiscovered`] drops whatever a stored index still holds that discovery did
    /// not name, and the list it compares against is a *snapshot* taken before the pass ingested
    /// anything -- so a file that appeared during the pass is not in it. The window is the length
    /// of a bootstrap, and what fits in it is exactly what makes files appear: a `git switch`, a
    /// `composer install`, a code generator.
    ///
    /// Recorded only inside that window -- see [`Self::discovering`] -- and cleared by
    /// [`Self::forget_undiscovered`] once it has kept them, because the next walk finds them on disk
    /// and names them itself. Both halves are what bound it to one pass's worth of changes. It used
    /// to be recorded always and cleared only by a pass, and a session runs one pass at startup and
    /// then perhaps never again: every save and every file a `git switch` touched stayed here for
    /// the life of the server. Found by `tests::a_session_gives_its_memory_back`, at 83 bytes a
    /// change.
    watched_since_discovery: Vec<PathBuf>,
    /// Whether a walk has run whose list [`Self::forget_undiscovered`] has not yet settled: the
    /// window in which [`Self::watched_since_discovery`] has anything to remember. Set by
    /// [`Self::discover`], which runs under a read guard -- hence an atomic -- and before the walk,
    /// so a change landing during the walk is remembered too. A change before it needs nothing:
    /// the walk finds it on disk.
    discovering: AtomicBool,
    /// The PHP standard library, materialized on disk. Empty only if extraction failed.
    stubs: StubSet,
    /// Where a written index goes, when anywhere.
    ///
    /// `None` means this workspace does not persist one, which is what a test wants: a fixture's root
    /// is a fresh temporary directory every run, so a real cache directory would collect a file per
    /// test and never reuse one. Same decision as the stub set -- touching a real cache directory
    /// belongs to whoever constructs the server, not to the workspace.
    cache_root: Option<PathBuf>,
    /// Why the standard library is missing, if it is. Surfaced as a warning rather than swallowed:
    /// without stubs every reference to `strlen` reports as undeclared, and a user who is not told
    /// why will read that as the server being broken.
    stub_error: Option<String>,
    skeleton: ProjectSkeleton,
    open: OpenSet,
    vfs: Vfs,
}

/// The lint rules in force, resolved once.
///
/// Owned rather than borrowed from a configuration value, because a caller wants a
/// [`LintSelection`](phrost_syntax_features::lint::LintSelection) and building one per request from a
/// file would mean reading the file per request.
#[derive(Clone, Debug)]
struct LintRules {
    categories: Vec<phrost_syntax_features::lint::LintCategory>,
    off: Vec<String>,
    on: Vec<String>,
}

impl Default for LintRules {
    /// The two categories that state a fact rather than a preference.
    ///
    /// Measured on a real project: all nine categories produce 6 855 findings against 508 from the
    /// index, and even `correctness` is dominated by one rule about `declare(strict_types=1)` that
    /// the project deliberately does not follow. These two produce 44 across 1 662 files, from six
    /// rules, and every one is something a reader wants to know.
    fn default() -> Self {
        Self {
            categories: vec![
                phrost_syntax_features::lint::LintCategory::Safety,
                phrost_syntax_features::lint::LintCategory::Security,
            ],
            off: Vec::new(),
            on: Vec::new(),
        }
    }
}

impl Workspace {
    /// A workspace over project code and the PHP standard library.
    ///
    /// The stub set is passed in rather than installed here so that the decision to touch a real
    /// cache directory belongs to whoever constructs the server. A test wants
    /// [`StubSet::empty`](phrost_stubs::StubSet::empty) and no disk at all.
    pub fn new(stubs: StubSet) -> Self {
        Self {
            stubs,
            cache_root: dirs::cache_dir().map(|dir| dir.join("phrost").join("index")),
            ..Self::default()
        }
    }

    /// The same, writing its index somewhere the caller chose. What a test uses.
    pub fn caching_in(stubs: StubSet, cache_root: impl Into<PathBuf>) -> Self {
        Self {
            stubs,
            cache_root: Some(cache_root.into()),
            ..Self::default()
        }
    }

    /// The same, reporting why the standard library is unavailable instead of carrying it.
    pub fn without_stubs(reason: String) -> Self {
        Self {
            stub_error: Some(reason),
            ..Self::default()
        }
    }

    /// Records what the client sent as `initializationOptions`, before the root is fixed.
    ///
    /// Separate from [`Self::set_root`] because the two arrive from different places -- the settings
    /// from the `initialize` request, the root from whichever of its fields carries one -- and because
    /// a caller with no editor at all (the checker, the perf sweep) must not have to pass `None` to
    /// say so. It has to be called first either way: the root is what makes the settings resolvable.
    pub fn set_editor_config(&mut self, settings: Option<serde_json::Value>) {
        self.editor_config = settings;
    }

    /// Fixes the project root and reads composer's autoload configuration.
    ///
    /// Configuration only: no PHP is read here, so a project whose `composer.json` is missing or
    /// broken still gets a working server over whatever it can discover.
    /// Returns the root as the layout resolved it, which is not always what was passed in.
    ///
    /// Handed back rather than left to be asked for, because three commands took the argument they
    /// were given and kept using it: a relative root is made absolute on the way in, so every path
    /// the index holds is absolute, and a caller still holding `.` printed whole paths where it
    /// meant short ones and looked for a file nobody had indexed.
    pub fn set_root(&mut self, root: PathBuf) -> &Path {
        let layout = ProjectLayout::new(root);
        let mut interner = Interner::new();

        self.php_floor = None;
        if let Ok(composer) = ComposerJson::load(&layout.root().join("composer.json")) {
            self.php_floor = composer.php_version();
            composer.apply_to(&mut interner, &mut self.autoload);
            self.is_an_application = composer.is_an_application();
            self.skeleton.set_is_an_application(self.is_an_application);
        }
        phrost_project::load_generated_autoload(&mut interner, &mut self.autoload, &layout);

        // Composer is not the only thing that decides where PHP lives, and the generic layer must
        // not be taught which frameworks exist -- so it asks. What comes back is ordinary autoload
        // configuration and extra file extensions; nothing here knows whose they are.
        // First, because three of its settings decide what is *indexed* rather than how a finding
        // is reported: which frameworks are on, and which directories are skipped or taken back.
        // The rest is applied at the end, where it always was -- see the comment there for why that
        // is the one place any of it lands.
        let (config, config_errors) =
            phrost_project::ProjectConfig::resolve(layout.root(), self.editor_config.as_ref());
        self.config_errors = config_errors;

        self.contributed_extensions.clear();
        self.contributed_config_extensions.clear();
        self.dependency_paths.clear();
        self.framework_sources.clear();
        self.configured_frameworks.clear();
        // Contributed per provider and added to rather than replaced, so a re-read has to drop the
        // previous run's before the providers speak again.
        self.skeleton.forget_run_supertypes();
        let mut excluded_paths = config.excluded_paths(layout.root());
        // Collected across every provider and installed once, because it has to be in place before
        // the first file is lowered: a declaration lowered without it carries no literal, and
        // nothing later would go back and look again.
        let mut keyed_attributes: Vec<&'static str> = Vec::new();
        let mut entered_arguments: Vec<&'static str> = Vec::new();
        // Before the providers and outside the loop: what PHP guarantees about its own standard
        // library holds whether or not a framework was detected, and the project this was found on
        // has none.
        self.skeleton.returns_mut().install_language_contracts();
        // Detected, plus whatever the project named. Added and not substituted: a project saying
        // `drupal` has said nothing about the `symfony` beside it. A provider reached twice
        // contributes twice, and every contribution is a set or a de-duplicated list, so naming one
        // that was already detected changes nothing.
        let mut providers = phrost_framework::detect(layout.root());
        if let Some(named) = config.framework.as_deref() {
            let (found, unknown) = phrost_framework::named(named);
            for provider in found {
                if !providers.iter().any(|seen| seen.id() == provider.id()) {
                    self.configured_frameworks.push(provider.id());
                    providers.push(provider);
                }
            }
            // Said out loud, because a misspelling here turns a framework's whole knowledge off and
            // looks exactly like the feature not working.
            if !unknown.is_empty() {
                // Not named after `phrost.toml`, because the same setting arrives from a client's
                // `initializationOptions` and naming the wrong file sends somebody to edit it.
                self.config_errors.push(format!(
                    "unknown framework {} -- known: {}",
                    unknown.join(", "),
                    phrost_framework::known_ids().join(", ")
                ));
            }
        }
        for provider in providers {
            self.install(
                provider,
                layout.root(),
                &mut interner,
                &mut keyed_attributes,
                &mut entered_arguments,
                &mut excluded_paths,
            );
        }
        // After the loop, because a framework's exclusions and the project's are one list to the
        // reader -- and the project's `include` outranks both.
        self.path_rules = phrost_project::paths::PathRules::new(
            excluded_paths,
            config.included_paths(layout.root()),
        );
        self.contributed_extensions.sort();
        self.contributed_extensions.dedup();
        self.contributed_config_extensions.sort();
        self.contributed_config_extensions.dedup();

        // Both halves of the index lower a file -- the skeleton for a closed one, the open set for
        // a buffer -- and they have to agree, or opening a file withdraws the keys its declarations
        // claimed.
        // And the property defaults a stated-properties rule reads, which lowering keeps only when
        // told to -- derived from the rules just installed, so the two cannot name different ones.
        let returns = self.skeleton.returns();
        let kept: Vec<Box<str>> = returns
            .stated_property_rules()
            .iter()
            .flat_map(|rule| rule.typed.iter().chain(&rule.listed).cloned())
            .chain(
                returns
                    .declared_inputs()
                    .iter()
                    .map(|rule| rule.property.clone()),
            )
            .chain(
                returns
                    .written_schema()
                    .iter()
                    .map(|rule| rule.table_property.clone()),
            )
            .chain(
                returns
                    .named_arguments()
                    .iter()
                    .filter_map(|rule| rule.from_property.clone()),
            )
            .collect();
        let listing: Vec<Box<str>> = self
            .skeleton
            .returns()
            .stated_property_rules()
            .iter()
            .flat_map(|rule| rule.listed_by_attribute.iter().cloned())
            .collect();
        let mixing: Vec<Box<str>> = self.skeleton.returns().mixin_methods().to_vec();
        let constructing: Vec<Box<str>> = self.skeleton.returns().construction_methods().to_vec();
        let constructed: Vec<Box<str>> = self
            .skeleton
            .returns()
            .constructed_entries()
            .iter()
            .map(|(class, _)| class.clone())
            .collect();
        let keyed = phrost_index::KeyedAttributes::of(keyed_attributes)
            .entering(entered_arguments)
            .keeping_defaults_of(kept)
            .keeping_lists_of(listing)
            .mixing_in_returns_of(mixing)
            .keeping_constructions(constructing, constructed);
        self.open.set_keyed_attributes(keyed.clone());
        self.skeleton.set_keyed_attributes(keyed);

        // After the frameworks, because a framework contributes autoload rules of its own -- Drupal
        // registers every module's namespace from a `.info.yml` and composer knows about none of
        // them. What the roots settle is whose scope a file's top level is; see
        // `ProjectSkeleton::file_scope_is_private`.
        self.skeleton
            .set_source_roots(self.autoload.namespace_roots());
        // The same directories with the prefix each stands for, which is what says whether a file
        // declares the namespace the autoloader will look for it under.
        self.skeleton.set_namespace_rules(
            self.autoload
                .psr4_rules()
                .map(|(prefix, dir)| (prefix.into(), dir.clone()))
                .collect(),
        );

        // Applied last, so that what the project says overrides what a framework assumed -- and
        // only what it says: a setting no source names keeps the default rather than resetting it.
        // Read at the top of this function, because `framework` and `files` decide what is indexed
        // and so have to be in hand before the providers and before discovery; everything from here
        // down is about how a finding is *reported*, and this is the one place any of it is applied,
        // which is what keeps the checker and the editor reporting the same rules.
        if let Some(style) = config.format.style.as_deref().and_then(FormatStyle::named) {
            self.format_style = style;
        }
        if let Some(categories) = config.lint.categories {
            self.lint.categories = categories
                .iter()
                .filter_map(|name| phrost_syntax_features::lint::LintCategory::named(name))
                .collect();
        }
        if let Some(off) = config.lint.off {
            self.lint.off = off;
        }
        if let Some(on) = config.lint.on {
            self.lint.on = on;
        }
        if let Some(project) = config.diagnostics.project {
            self.diagnose = match project {
                true => Diagnose::Project,
                false => Diagnose::OpenOnly,
            };
        }
        // After the providers, and installed through the same call they use, so a project's entry and
        // a framework's are the same kind of fact from here on. A framework names its own accessors;
        // a project that wraps them loses the completion and the near-miss report unless it says so,
        // and nothing in the seam may name a customer's class.
        // Which pages a template included by a computed name is included from, which only the
        // project can say.
        for context in config.twig.context.into_iter().flatten() {
            self.skeleton
                .returns_mut()
                .add_twig_context(&context.templates, &context.from);
        }
        for argument in config.catalog.argument.into_iter().flatten() {
            self.skeleton.returns_mut().bind_argument(
                &argument.class,
                &argument.method,
                argument.at,
                &argument.suffix,
                &argument.catalog,
            );
        }

        // The skeleton derives the short display path itself, from this.
        self.skeleton.set_root(layout.root());
        self.layout.insert(layout).root()
    }

    /// Why the standard library is missing, if it is.
    pub fn stub_error(&self) -> Option<&str> {
        self.stub_error.as_deref()
    }

    pub fn stub_count(&self) -> usize {
        self.stubs.len()
    }

    /// Ids of the frameworks in force, for the startup log.
    pub fn frameworks(&self) -> &[&'static str] {
        &self.frameworks
    }

    /// The same, said the way a log should say it: which are on, and which the project asked for.
    ///
    /// Worth distinguishing because the two failures look identical from outside. A framework that
    /// was *not* detected explains a project whose services and hooks resolve to nothing; a
    /// framework that was only detected because the configuration named it explains one where a
    /// contract fires somewhere it should not. Neither is visible if the log says only the ids.
    /// How many paths the project and its frameworks take out of the walk, for the startup log.
    pub fn excluded_count(&self) -> usize {
        self.path_rules.excluded().len()
    }

    /// How many it takes back.
    pub fn included_count(&self) -> usize {
        self.path_rules.included().len()
    }

    pub fn framework_summary(&self) -> String {
        if self.frameworks.is_empty() {
            return "none detected".to_owned();
        }
        self.frameworks
            .iter()
            .map(
                |id| match self.configured_frameworks.iter().any(|named| named == id) {
                    true => format!("{id} (configured)"),
                    false => (*id).to_owned(),
                },
            )
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn layout(&self) -> Option<&ProjectLayout> {
        self.layout.as_ref()
    }

    /// The house style a formatting request reprints in.
    /// The rules a reader is shown without asking for them.
    pub fn editor_lint(&self) -> phrost_syntax_features::lint::LintSelection<'_> {
        phrost_syntax_features::lint::LintSelection {
            categories: &self.lint.categories,
            off: &self.lint.off,
            on: &self.lint.on,
        }
    }

    /// The checks a project asked for beyond the ones that state a fact.
    ///
    /// The same list the syntax rules read, because a person naming a rule in `phrost.toml` does not
    /// care which engine implements it.
    pub fn asked_for(&self) -> phrost_analysis::AskedFor<'_> {
        phrost_analysis::AskedFor {
            codes: &self.lint.on,
            off: &self.lint.off,
        }
    }

    /// The rules the project named, either way.
    ///
    /// For a caller that chooses the categories itself -- the checker takes them on the command line
    /// -- and must still honour what the project said: a rule switched off is off however the
    /// categories were chosen, or the checker reports what the editor does not.
    /// The lint categories in force: what `phrost.toml` asked for, or the two that state a fact.
    ///
    /// Read by the checker as well as by the editor. A command line that started from an empty list
    /// instead reported nothing from the syntax rules while the editor underlined their findings in
    /// the same file, which is the divergence
    /// [`Sweep::lint`](../../../phrost_cli/sweep/struct.Sweep.html) already warns about for the
    /// *named* rules -- the categories were the half still taken from somewhere else.
    pub fn lint_categories(&self) -> &[phrost_syntax_features::lint::LintCategory] {
        &self.lint.categories
    }

    pub fn named_lint_rules(&self) -> (&[String], &[String]) {
        (&self.lint.off, &self.lint.on)
    }

    /// Why a configuration source was ignored, one reason per source that was not understood.
    ///
    /// A list rather than one message because there are two sources and either can be wrong on its
    /// own: a person told only about the first would fix it, restart, and find the second still there.
    pub fn config_errors(&self) -> &[String] {
        &self.config_errors
    }

    pub fn format_style(&self) -> FormatStyle {
        self.format_style
    }

    /// What a format request reprints for: the style, and the oldest PHP the result has to run on.
    pub fn format_target(&self) -> phrost_syntax_features::format::FormatTarget {
        phrost_syntax_features::format::FormatTarget {
            style: self.format_style,
            php: self.php_floor,
        }
    }

    /// The oldest PHP the project says it runs on, for the syntax rules that are advice to write
    /// something a version added.
    pub fn php_floor(&self) -> Option<phrost_model::PhpVersion> {
        self.php_floor
    }

    pub fn autoload(&self) -> &AutoloadMap {
        &self.autoload
    }

    /// Every file worth reading, discovered without holding a lock on anything.
    ///
    /// Both kinds from one walk. A second walk for the configuration would cost what the first
    /// does -- the cost of a walk is `opendir` per directory and both want the same directories --
    /// so the extensions go in together and the results are told apart by name on the way out.
    pub fn discover(&self) -> Discovered {
        // Ordered by the lock rather than by this: a watched change is applied under the write
        // guard, so it lands either wholly before this read section or wholly after it.
        self.discovering.store(true, Ordering::Relaxed);
        self.layout
            .as_ref()
            .map(|layout| {
                let mut extensions = self.php_extensions().collect::<Vec<_>>();
                extensions.extend(CONFIG_EXTENSIONS);
                // In the walk that is already happening rather than a second one: a template is
                // found the way a configuration file is, and paying another pass over the tree at
                // every start for a fact one project-level check reads is what §3 refuses.
                extensions.extend(phrost_project::TEMPLATE_EXTENSIONS);
                // One list: both are paths to walk with ignore files off, and which of them a file
                // came from decides nothing -- the first pass has already said whose code it is.
                let mut paths = self.autoload.source_roots();
                paths.extend(self.framework_sources.iter().cloned());
                let mut found = Discovered::default();
                for file in
                    discover_php_files(layout, &extensions, &paths, &self.path_rules, &self.stubs)
                {
                    // The project's own only, for both of the two that are not PHP, which is the
                    // scope a reference sweep has: a dependency's configuration names a
                    // dependency's classes and a dependency's templates call its own methods, and
                    // neither is something anybody acts on from here.
                    if is_config_extension(&file.path) {
                        if file.origin == SourceOrigin::User {
                            found.configuration.push(file.path);
                        }
                    } else if is_template_extension(&file.path) {
                        if file.origin == SourceOrigin::User {
                            found.templates.push(file.path);
                        }
                    } else {
                        found.php.push((file.path, file.origin));
                    }
                }
                found
            })
            .unwrap_or_default()
    }

    /// Reads where the project's configuration names a class, and records it.
    ///
    /// Once per bootstrap, after the frameworks are installed -- the rules that say which key in
    /// which file names a class are theirs, and there is nothing to look for until they are in.
    /// Only the files some rule covers are opened, which is what makes it affordable: **53 of a
    /// real project's 1 551**, and the test for it is the file's name. Reading them all instead
    /// measured 54 ms of every start against 5.4 ms, for the same 441 sites.
    /// Every word the project's own templates write, for the dead-code pass.
    ///
    /// A Twig template calls a PHP method and no PHP file names it -- and often under another name
    /// again, since Twig reads `{{ node.title }}` as `$node->title`, `$node->title()`,
    /// `$node->getTitle()`, `$node->isTitle()` or `$node->hasTitle()`, whichever the object has. So
    /// the name a template writes is frequently not the name the method carries, and the getter
    /// spellings are recorded beside every word.
    ///
    /// Words rather than parsed calls, which is coarse and is the direction this has to be coarse
    /// in: the set is only ever consulted to *withhold* a finding, exactly as the member half of
    /// the dead-code rule already is. Twelve of the reference project's 124 unreached members are
    /// written in one, `{{ trade.status.getDescription }}` among them.
    ///
    /// Installed rather than cached, beside the configuration sites and for the same reason: it is
    /// read from files the index never holds, so there is nothing of it in a stored skeleton.
    pub fn install_template_words(&mut self, files: &[PathBuf]) {
        let mut words: std::collections::HashSet<Box<str>> = std::collections::HashSet::new();
        for path in files {
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            // And what each includes, from the same read: the one included finds who includes it
            // from these, to read the variables it is handed.
            self.skeleton
                .set_twig_includes(path, phrost_view::twig::included_template_names(&text));
            for word in text.split(|byte: char| !byte.is_ascii_alphanumeric() && byte != '_') {
                // Under three characters a word is `if`, `as` or a number, and no member name this
                // is asked about is that short either.
                if word.len() < 3 {
                    continue;
                }
                let word = word.to_ascii_lowercase();
                for prefix in ["get", "is", "has"] {
                    words.insert(format!("{prefix}{word}").into_boxed_str());
                }
                words.insert(word.into_boxed_str());
            }
        }
        self.skeleton.set_template_words(words);
    }

    pub fn install_config_sites(&mut self, files: &[PathBuf]) {
        let mut sites = Vec::new();
        for path in files {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            // Before the read, not after: a project's own configuration is over a thousand files
            // and a rule names a few dozen of them.
            if !self.project_view().covers_configuration(name) {
                continue;
            }
            // The buffer when the editor has one, because a file being edited is what the person
            // asking is looking at -- the same rule every other reader here follows.
            let text = match self.configs.get(path) {
                Some(held) => held.text.to_string(),
                None => match std::fs::read(path) {
                    // Lossy, for the reason a composer-generated file is read that way: a stray
                    // byte in a comment must not drop the file.
                    Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                    Err(_) => continue,
                },
            };
            let shared: Arc<Path> = Arc::from(path.as_path());
            sites.extend(
                self.project_view()
                    .config_class_sites(name, &text)
                    .into_iter()
                    .map(|site| phrost_index::ConfigSite {
                        path: Arc::clone(&shared),
                        name: site.name,
                        range: site.range,
                        member: site.member,
                    }),
            );
        }
        self.skeleton.set_config_sites(sites);
    }

    /// Every place the project's configuration names `fqn`.
    pub fn config_sites_naming(&self, fqn: &str) -> Vec<(PathBuf, phrost_model::TextRange)> {
        self.skeleton
            .config_sites_naming(fqn)
            .map(|site| (site.path.to_path_buf(), site.range))
            .collect()
    }

    /// Every class the project's configuration names, lowercased.
    ///
    /// For a reader asking what reaches a declaration rather than where: a service's `class:` is
    /// as good a reason for a class to exist as a `new` is.
    pub fn config_named_classes(&self) -> Vec<String> {
        self.skeleton
            .config_named_classes()
            .map(|name| name.to_ascii_lowercase())
            .collect()
    }

    /// Every member the project's configuration names, lowercased -- a route's `Class::action`.
    pub fn config_named_members(&self) -> Vec<String> {
        self.skeleton.config_named_members().collect()
    }

    /// Whether a class above this one also declares the member -- see `member_comes_from_above`.
    pub fn member_comes_from_above(
        &self,
        class_fqn: &str,
        kind: phrost_view::MemberKind,
        name: &str,
    ) -> bool {
        self.project_view()
            .member_comes_from_above(class_fqn, kind, name)
    }

    /// Whether composer calls this package an application rather than a library.
    pub fn is_an_application(&self) -> bool {
        self.is_an_application
    }

    /// Takes on what analysing one file found it reaches.
    ///
    /// Under a shared lock, because it is derived state -- see `phrost_index::Reached`. Called
    /// from both places a file is checked, which between them cover every file the project owns:
    /// the project-wide pass for the closed ones and the buffer check for the open ones.
    pub fn record_reached(&self, file: FileId, reaches: phrost_index::Reaches) {
        self.skeleton.record_reached(file, reaches);
    }

    /// Said once a pass has covered every file the project owns, which is what lets the unused
    /// check read an absence as an answer rather than as "not swept yet".
    pub fn mark_reached_complete(&self) -> bool {
        self.skeleton.mark_reached_complete()
    }

    /// Whether any check reads the record of what the project reaches.
    ///
    /// Asked before the pass that fills it is run a second time: gathering is free -- it rides on
    /// an analysis being done anyway -- and *judging again* is a whole pass, which is not worth
    /// paying for on a project that never switched the check on.
    /// Whether anything in the project reaches this member of this class -- see `Reached`.
    pub fn member_is_reached(&self, owner: &str, name: &str) -> bool {
        self.skeleton.member_is_reached(owner, name)
    }

    /// Whether something that is not a call written in PHP reaches this member.
    ///
    /// The one assembly of that rule -- see
    /// [`member_is_reached_without_a_call`](phrost_index::ProjectSkeleton::member_is_reached_without_a_call).
    pub fn member_is_reached_without_a_call(&self, owner: &str, name: &str) -> bool {
        self.skeleton.member_is_reached_without_a_call(owner, name)
    }

    /// Whether the project's configuration writes this member name anywhere.
    pub fn config_names_member(&self, name: &str) -> bool {
        self.skeleton.config_names_member(name)
    }

    pub fn reads_what_is_reached(&self) -> bool {
        self.lint.on.iter().any(|code| code == "unused-declaration")
    }

    /// Whether a framework finds this class by scanning rather than being told about it.
    pub fn is_discovered(&self, fqn: &str) -> bool {
        self.skeleton.is_discovered(fqn)
    }

    /// Every class a framework builds because of what it inherits, lowercased.
    pub fn discovered_by_inheritance(&self) -> std::collections::HashSet<String> {
        self.skeleton.discovered_by_inheritance()
    }

    /// Every place the project's configuration calls `class::member`.
    pub fn config_sites_calling(
        &self,
        class: &str,
        member: &str,
    ) -> Vec<(PathBuf, phrost_model::TextRange)> {
        self.skeleton
            .config_sites_calling(class, member)
            .map(|(site, at)| (site.path.to_path_buf(), at))
            .collect()
    }

    /// Indexes one file from disk.
    ///
    /// A read failure is not an error worth reporting: a file can vanish between discovery and
    /// ingestion, and the right response is to index the rest of the project.
    pub fn ingest_from_disk(&mut self, path: &Path, origin: SourceOrigin) {
        let Ok((source, fingerprint)) = SourceText::read_with_fingerprint(path) else {
            return;
        };
        let file = self.vfs.register(path, origin);
        match self.skeleton.as_php(path, source.text()) {
            Some(php) => self.skeleton.ingest(file, path, origin, &php),
            None => self.skeleton.ingest(file, path, origin, source.text()),
        }
        self.refresh_renderer(file);
        // Read from disk, so the next start may trust the metadata for this file too.
        self.skeleton.set_fingerprint(file, fingerprint);
    }

    /// Takes a batch of discovered files into the index, reading and lowering them in parallel.
    ///
    /// The batch rather than the file is the unit because parallelism needs one: reading and
    /// lowering are 90 % of a cold start and touch nothing shared, while the claim that follows
    /// touches the interner, the catalog and the member tables. Registering each file's id here
    /// keeps the vfs out of the index crate, and costs 16 ms of the 2.92 s indexing thirty thousand
    /// files used to take. See [`phrost_index::ProjectSkeleton::ingest_batch`].
    ///
    /// `reuse` asks each file whether the index already describes exactly its bytes, which is what
    /// separates a warm start from a cold one. The read happens either way -- indexing has to read
    /// a file to parse it and checking has to read it to hash it -- so what a hit saves is the
    /// parse, the lowering and the collecting, which is most of what a cold start costs.
    ///
    /// Compared by content hash and not by modification time. A timestamp lies in both directions:
    /// a checkout restores an old file with a new time, and a build writes the same bytes back. A
    /// wrong answer here is a stale index, which is the worst thing this project can ship.
    ///
    /// Returns how many rows were reused, and how that was known.
    pub fn ingest_batch(
        &mut self,
        batch: &[(PathBuf, SourceOrigin)],
        reuse: bool,
    ) -> phrost_index::Reused {
        let registered: Vec<FileId> = batch
            .iter()
            .map(|(path, origin)| self.vfs.register(path, *origin))
            .collect();
        let files: Vec<phrost_index::BatchFile<'_>> = batch
            .iter()
            .zip(&registered)
            .map(|((path, origin), &file)| phrost_index::BatchFile {
                file,
                path,
                origin: *origin,
            })
            .collect();
        self.skeleton.ingest_batch(&files, reuse)
    }

    /// Drops whatever a loaded index still holds for files the project no longer has.
    ///
    /// A cache describes a checkout that has since moved on. A file that was deleted has to leave
    /// the catalog with it, or its class keeps resolving to a path that is not there.
    pub fn forget_undiscovered(&mut self, discovered: &[(PathBuf, SourceOrigin)]) {
        let live = discovered
            .iter()
            .filter_map(|(path, _)| self.vfs.file_id(path))
            // A buffer the editor has open is live whether discovery returned it or not. Discovery
            // is autoload-driven, so a scratch file outside every source root is not in it -- and
            // dropping one here is how a file's own classes came to report as undefined in the very
            // file that declares them.
            .chain(self.vfs.files().filter(|file| self.open.contains(*file)))
            // And a file the watcher brought in since the snapshot was taken, which is the same
            // kind of fact as an open buffer: something the index holds that discovery's list
            // predates. See `watched_since_discovery`.
            .chain(
                self.watched_since_discovery
                    .iter()
                    .filter_map(|path| self.vfs.file_id(path)),
            )
            .collect::<Vec<_>>();
        self.skeleton.retain_files(&live);
        // Their purpose is served: the next walk finds them on disk and names them itself, so
        // keeping them would be a list that only grows. Cleared here rather than by `discover`,
        // which is `&self` on purpose -- it runs under a read guard so a walk of thirty thousand
        // files does not block the keystroke waiting to write.
        self.watched_since_discovery.clear();
        *self.discovering.get_mut() = false;
    }

    /// Puts every open buffer back into the index, from the text the editor last sent.
    ///
    /// For the steps that replace the skeleton underneath the editor. Loading a cache adopts a
    /// stored one, and forgetting the undiscovered drops what discovery did not name; a buffer
    /// opened before either is gone from the index afterwards while it is still on screen, and the
    /// symptom is a file whose own declarations read as undefined until one character is typed --
    /// that keystroke being the edit that ingests it again.
    ///
    /// From the open set rather than from disk, for the two reasons that matter: the buffer may hold
    /// unsaved edits, which are what the person is looking at, and it is already lowered.
    pub fn reingest_open_documents(&mut self) {
        let open = self
            .vfs
            .files()
            .filter(|file| self.open.contains(*file))
            .collect::<Vec<_>>();

        for file in open {
            let path = self.vfs.path(file).to_path_buf();
            let Some(text) = self
                .vfs
                .open_document(file)
                .map(|document| document.source().shared_text())
            else {
                continue;
            };
            let origin = self
                .layout
                .as_ref()
                .map_or(SourceOrigin::User, |layout| layout.source_origin(&path));

            match self.open.get(file).filter(|_| origin.wants_full_analysis()) {
                Some(open) => self.skeleton.ingest_lowered(
                    file,
                    &path,
                    origin,
                    &text,
                    FileStamp::of(text.as_bytes()),
                    open.hir(),
                ),
                None => self.skeleton.ingest(file, &path, origin, &text),
            }
        }
    }

    /// Reads a previously written index, if there is one for this project.
    ///
    /// Loaded whole and then checked file by file, rather than trusted: the ids, the interner and the
    /// type table all have to be the ones the stored member rows were written against, so the loaded
    /// index *is* the index -- and every file in it is re-read and re-ingested unless its bytes hash
    /// the same. A cache for a checkout that has entirely moved on costs one read and saves nothing,
    /// which is the right shape for the failure.
    pub fn load_cache(&mut self) {
        let Some(path) = self.cache_path() else {
            return;
        };
        // Streamed rather than read whole. A 46 MB `Vec` to decode from is an allocation macOS
        // keeps rather than returns, and `vmmap` showed it as most of the 56.6 MB of
        // empty-but-dirty large regions a warm server was holding. Not buffered here: the codec's
        // reader keeps its own window over the source and reads it in large pieces.
        let Ok(mut source) = std::fs::File::open(&path) else {
            return;
        };
        let Ok(len) = source.metadata().map(|found| found.len()) else {
            return;
        };

        // Ids come from this session's VFS, so paths are registered as they are read -- which is what
        // makes a loaded entry findable by the id the ingest loop will use for the same path.
        let origin = |path: &Path| {
            self.layout
                .as_ref()
                .map_or(SourceOrigin::User, |layout| layout.source_origin(path))
        };
        let vfs = &mut self.vfs;
        let Some(loaded) = ProjectSkeleton::decode(&mut source, len, build_stamp(), |path| {
            vfs.register(path, origin(path))
        }) else {
            return;
        };

        // Adopted rather than assigned: what a cache holds is what files said, and the framework
        // contracts installed by `set_root` are not that. An assignment here would drop them, and
        // nothing would report it -- the project would simply be checked as if no framework were
        // present, on exactly the runs where the cache worked.
        self.skeleton.adopt(loaded);
        if let Some(layout) = self.layout.as_ref() {
            // The stored root is this project's, but the display paths are derived from it and the
            // live layout is the authority on where the project is.
            self.skeleton.set_root(layout.root());
        }
    }

    /// Clears out cached indexes that can never be read again, and caps what is left.
    ///
    /// On the way in, not on the way out: a server that is killed never gets to tidy up, and being
    /// killed is the normal end of an editor session. Reported by the caller, because a cache that
    /// quietly grew to most of a gigabyte is exactly the thing nobody notices.
    pub fn prune_cache(&self) -> phrost_index::cache::Pruned {
        match self.cache_root.as_ref() {
            Some(root) => phrost_index::cache::prune(root, build_stamp(), CACHED_PROJECTS),
            None => phrost_index::cache::Pruned::default(),
        }
    }

    /// Writes the index for next time, and reports whether it went.
    ///
    /// A failure is not worth telling anyone about: a cache that cannot be written costs the seconds
    /// it was going to save, and a read-only or full disk is not this program's business.
    pub fn store_cache(&self) -> bool {
        let Some(path) = self.cache_path() else {
            return false;
        };
        phrost_index::cache::store(&path, |sink| self.skeleton.write_to(sink, build_stamp()))
            .is_ok()
    }

    /// Where this project's index lives, keyed by its root.
    ///
    /// Under the user's cache directory rather than in the project: an index is derived data in this
    /// build's private format, and a directory `git status` reports on is the wrong home for either.
    fn cache_path(&self) -> Option<PathBuf> {
        // The project's identity rather than its root: two spellings of one directory are one
        // project, and this is the only place that has to know it. See `ProjectLayout::identity`.
        let root = self.layout.as_ref()?.identity();
        let digest = blake3::hash(root.to_string_lossy().as_bytes()).to_hex();
        Some(
            self.cache_root
                .as_ref()?
                .join(format!("{}.phrost", &digest[..16])),
        )
    }

    pub fn mark_ready(&mut self) {
        self.skeleton.mark_ready();
    }

    pub fn is_ready(&self) -> bool {
        self.skeleton.state().is_ready()
    }

    pub fn indexed_file_count(&self) -> usize {
        self.skeleton.file_count()
    }

    // --- document sync ---

    /// Records an opened or changed buffer, and re-indexes the file from that text.
    ///
    /// Both the skeleton and the open set are updated from the same string, so the two tiers cannot
    /// describe different revisions of one file.
    ///
    /// A revision older than the one already held is ignored, and the return value says so. The
    /// protocol promises ordered notifications per document, so this should never fire — which is
    /// exactly why it is worth having: if it ever does, the alternative is an index that has quietly
    /// gone backwards while the editor shows something else, and nothing downstream could tell.
    pub fn set_document(
        &mut self,
        path: &Path,
        text: Arc<str>,
        version: DocumentVersion,
    ) -> DocumentUpdate {
        // A client may be configured to send this server every buffer of a language it does not
        // read. Zed's extension registers YAML alongside PHP -- deliberately, because Drupal and
        // Symfony declare their services there and a later feature will want them -- and until
        // something reads YAML, holding one costs a parse of non-PHP text on every keystroke and a
        // buffer's worth of resident bytes that the memory rule says should be PHP.
        //
        // Held as text and nothing more, because the two tiers are updated from this one string: a
        // file ingested here is a file the skeleton claims declarations for, and a `services.yml`
        // declares no PHP. What reads it is `IndexView::config_reference_at`, which takes the text
        // and an offset and asks the catalogs -- no parse, no HIR, no claim on any name.
        if !self.is_indexable(path) {
            if !self.reads_configuration(path) {
                return DocumentUpdate::Ignored;
            }
            if let Some(held) = self.configs.get(path)
                && held.version > version
            {
                return DocumentUpdate::Stale { held: held.version };
            }
            // What a template includes is read from the buffer while it is open, so a part it
            // starts including finds its variables without waiting for the next start.
            if self.project_view().covers_configuration(
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(""),
            ) && path.extension().is_some_and(|ending| ending == "twig")
            {
                self.skeleton
                    .set_twig_includes(path, phrost_view::twig::included_template_names(&text));
            }
            self.configs
                .insert(path.to_path_buf(), ConfigBuffer { text, version });
            // A template's variables are read where the arrays rendering it are written, so those
            // files are lowered beside it while it is open -- as they are for one read as PHP.
            self.settle_renderers();
            return DocumentUpdate::Held;
        }
        // A template is held as the PHP its dialect makes of it, which is as long as the buffer and
        // keeps its lines -- so every position the client sends means the same thing in both.
        let written = Arc::clone(&text);
        let text: Arc<str> = match self.skeleton.as_php(path, &text) {
            Some(php) => Arc::from(php),
            None => text,
        };

        // Whatever discovery decided, and a fresh guess only for a file discovery never saw.
        // `source_origin` reads the `vendor/` prefix and nothing else -- deliberately, because
        // anything cleverer guesses at a layout composer lets a project choose -- so recomputing it
        // here **demoted** every file whose origin came from somewhere richer. A stub lives in a
        // cache directory outside the project entirely and came back `User`: body-lowered against
        // the memory rule and fully checked, so cmd-clicking into `date.php` published 222
        // `doc-missing-tags` about a file nobody can edit. Drupal's core, outside `vendor/` and
        // marked by the dependency walk, came back `User` the same way.
        // Whatever discovery decided, and a fresh guess only for a file discovery never saw.
        // `source_origin` reads the `vendor/` prefix and nothing else -- deliberately, because
        // anything cleverer guesses at a layout composer lets a project choose -- so recomputing it
        // here **demoted** every file whose origin came from somewhere richer. A stub lives in a
        // cache directory outside the project entirely and so came back `User`: body-lowered
        // against the memory rule and fully checked, which is why cmd-clicking into `date.php`
        // published 222 `doc-missing-tags` about a file nobody can edit.
        let origin = match self.vfs.file_id(path) {
            Some(known) => self.vfs.origin(known),
            None => self
                .layout
                .as_ref()
                .map_or(SourceOrigin::User, |layout| layout.source_origin(path)),
        };
        let file = self.vfs.register(path, origin);

        if let Some(open) = self.vfs.open_document(file)
            && open.version() > version
        {
            return DocumentUpdate::Stale {
                held: open.version(),
            };
        }

        // Opened before ingested, and ingested from what opening produced. Both tiers want this
        // file's HIR with bodies, and lowering the same text twice is the whole cost of opening a
        // buffer paid over again. Only for an origin ingest would have body-lowered anyway: an open
        // vendor file keeps the declaration-only path, or the memory rule would start depending on
        // which files somebody happens to have open.
        self.open.open(file, Arc::clone(&text));
        let source = SourceText::new(Arc::clone(&text));
        match self.open.get(file).filter(|_| origin.wants_full_analysis()) {
            Some(open) => {
                self.skeleton
                    .ingest_lowered(file, path, origin, &text, source.stamp(), open.hir())
            }
            None => self.skeleton.ingest(file, path, origin, &text),
        }
        self.vfs.open(file, source, version);
        if self.is_template(path) {
            self.written_templates.insert(path.to_path_buf(), written);
        }
        self.settle_renderers();

        DocumentUpdate::Applied
    }

    /// Forgets a buffer.
    ///
    /// Nothing else happens: no disk read, no re-analysis. The skeleton already knows what the file
    /// declares, so closing is pure forgetting — the previous attempt did a synchronous read here
    /// under a write lock, which is where "closing a file hangs the editor" came from.
    pub fn close_document(&mut self, path: &Path) {
        self.configs.remove(path);
        self.written_templates.remove(path);
        if let Some(file) = self.vfs.file_id(path) {
            self.open.close(file);
            self.vfs.close(file);
        }
        self.settle_renderers();
    }

    /// Whether any framework said it reads a file named like this one that is not PHP: a
    /// configuration file, or a Twig template its loader names.
    ///
    /// The gate on holding a buffer at all, and it is the framework's own list rather than a
    /// hardcoded `.yml`: a project with no framework detected holds nothing, and one whose framework
    /// reads five kinds of file holds those five. So a client that routes every YAML buffer here --
    /// which Zed's extension does -- costs a suffix comparison per open and no bytes.
    fn reads_configuration(&self, path: &Path) -> bool {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        self.project_view().covers_configuration(name)
    }

    /// The text of a template held beside an open one: a template including it.
    pub fn held_template_text(&self, path: &Path) -> Option<&str> {
        self.open.template_text(path)
    }

    /// The text of an open configuration buffer, if this is one.
    pub fn config_text(&self, path: &Path) -> Option<&Arc<str>> {
        self.configs.get(path).map(|held| &held.text)
    }

    // --- changes made outside the editor ---

    /// Applies one filesystem change the client reported.
    ///
    /// This is the path that keeps the index honest across a `git switch`, a `composer install`, or
    /// a file written by a build script. Without it the index silently describes a checkout that no
    /// longer exists, and every answer afterwards is about the wrong code — with nothing to hint at
    /// why.
    pub fn apply_watched_change(&mut self, path: &Path, removed: bool) -> WatchedOutcome {
        // Composer's own files decide where all the code is and which namespaces map where. A
        // change to one of them invalidates discovery itself, not one file's contents, so it is
        // reported for the caller to act on rather than patched up here -- and reported apart from
        // the rest, because the caller has to ask whether composer is still running.
        if Self::is_composer_file(path) || self.is_dependency_directory(path) {
            return WatchedOutcome::ComposerChanged;
        }
        if self.is_configuration(path) {
            return WatchedOutcome::ConfigurationChanged;
        }

        if !self.is_indexable(path) {
            // A template: what it includes, and its text where one open includes it. An open
            // buffer is the editor's and is left alone, as an open PHP file is.
            if is_template_extension(path) && !self.configs.contains_key(path) {
                self.refresh_template(path);
            }
            return WatchedOutcome::Ignored;
        }

        // An open buffer is what the person is looking at, and the editor owns its contents. A disk
        // change underneath it is not ours to apply: doing so would replace what is on screen with
        // what is on disk, and the editor will send `didChange` itself if it reloads.
        if let Some(file) = self.vfs.file_id(path)
            && self.open.contains(file)
        {
            return WatchedOutcome::Ignored;
        }

        if removed {
            if let Some(file) = self.vfs.file_id(path) {
                self.skeleton.remove(file);
                return WatchedOutcome::Removed;
            }
            return WatchedOutcome::Ignored;
        }

        let origin = self
            .layout
            .as_ref()
            .map_or(SourceOrigin::User, |layout| layout.source_origin(path));
        self.ingest_from_disk(path, origin);
        // Remembered until the walk being settled names it -- see the field. Without this the tail
        // of a bootstrap running beside the watcher drops the file again, and a class that is there
        // on disk reads as undefined with nothing to correct it. Outside a pass there is no tail to
        // survive, and nothing to remember.
        if *self.discovering.get_mut() {
            self.watched_since_discovery.push(path.to_path_buf());
        }
        WatchedOutcome::Reindexed
    }

    /// Whether a path is a file other than composer's that decides how the project is read.
    ///
    /// A framework's, and this project's own `phrost.toml`. The first is here because the
    /// contributions are read once, by `set_root`: a service id added to a `services.yml`, a
    /// route added to a `routing.yml` or a module added to the project is invisible to every answer
    /// until the providers are asked again, and a person who has just written the id reads that as
    /// the server not knowing about their code. Nothing smaller will do -- a provider reads its
    /// whole docroot to answer one question, so there is no such thing as re-reading one of its
    /// files.
    ///
    /// The second is here because a setting that only takes effect on a restart is a setting
    /// people believe does not work: the file is read
    /// once by [`Self::set_root`], and until it was watched a person switching a rule off saw nothing
    /// change and had no way to tell that from the rule being broken. The editor's
    /// `initializationOptions` never had the problem, because a client re-sends those by restarting
    /// the server.
    ///
    /// Only the one at the root, because that is the only one read. A `phrost.toml` deeper in the
    /// tree is somebody else's file and must not cost a reindex.
    fn is_configuration(&self, path: &Path) -> bool {
        if self.is_framework_configuration(path) {
            return true;
        }
        path.file_name()
            .is_some_and(|name| name == phrost_project::CONFIG_FILE)
            && self
                .layout
                .as_ref()
                .and_then(|layout| path.parent().map(|parent| parent == layout.root()))
                .unwrap_or(false)
    }

    /// Whether a framework reads this file to decide what it contributes.
    ///
    /// By extension and not by name, for the reason
    /// [`configuration_extensions`](phrost_framework::FrameworkProvider::configuration_extensions)
    /// gives: the names are spread across the provider's own modules and a second list of them
    /// would go stale. An excluded path is still excluded -- a generated cache full of YAML is not
    /// somebody declaring a service.
    fn is_framework_configuration(&self, path: &Path) -> bool {
        !self.contributed_config_extensions.is_empty()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    self.contributed_config_extensions
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(extension))
                })
            && !self.is_excluded(path)
    }

    /// Every extension that holds PHP: the language's own, plus what a framework added.
    fn php_extensions(&self) -> impl Iterator<Item = &str> {
        PHP_EXTENSIONS
            .iter()
            .copied()
            .chain(self.contributed_extensions.iter().map(String::as_str))
    }

    /// Whether `path` is a template a framework reads as PHP: read for what it reaches, and not
    /// checked, since a finding there would be about markup turned into PHP rather than about code
    /// somebody wrote.
    pub fn is_template(&self, path: &Path) -> bool {
        self.skeleton.is_template(path)
    }

    /// Whether a path has an extension this project indexes.
    pub fn is_indexable(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                self.php_extensions()
                    .any(|candidate| candidate.eq_ignore_ascii_case(extension))
            })
            && !self.is_excluded(path)
    }

    /// Whether this path is one the project does not own -- see `PathRules`.
    ///
    /// The same predicate discovery walks with, so a directory that is skipped is also a buffer
    /// that is refused. Two copies of it would let a file be indexed and then declined.
    fn is_excluded(&self, path: &Path) -> bool {
        self.path_rules.skips(path)
    }

    /// Glob patterns worth asking the client to watch.
    ///
    /// Derived from the extensions this project indexes, so a framework that contributed one gets
    /// watched too without anything here knowing which framework it was.
    pub fn watch_globs(&self) -> Vec<String> {
        let mut globs = self
            .php_extensions()
            .map(|extension| format!("**/*.{extension}"))
            .collect::<Vec<_>>();

        globs.push("**/composer.json".to_owned());
        globs.push("**/composer.lock".to_owned());
        // The dependency directory itself, appearing or going: see `is_dependency_directory`. A
        // glob and not a rooted pattern, because a client that roots a watch at a directory loads
        // everything under it -- and the directory's own entry is one every client already keeps.
        if let Some(name) = self
            .layout
            .as_ref()
            .and_then(|layout| layout.vendor_dir().file_name())
            .and_then(|name| name.to_str())
        {
            globs.push(format!("**/{name}"));
        }
        // Not composer's records under `vendor/`: a glob that starts with `**` is one a client may
        // answer from the tree it already keeps, and an ignored `vendor/` is not in it. Those are
        // watched from the directory they live in -- see `composer_records`.
        // What a framework said it reads, and nothing when none did: a project with no framework
        // must not ask its client to report every YAML it happens to contain, and one with a
        // framework has to hear about the file that declares its services.
        globs.extend(
            self.contributed_config_extensions
                .iter()
                .map(|extension| format!("**/*.{extension}")),
        );
        // Not `**/`, unlike composer's: a nested `composer.json` really does decide where code lives,
        // and a nested `phrost.toml` is read by nothing.
        globs.push(phrost_project::CONFIG_FILE.to_owned());
        globs
    }

    /// Forgets the indexed project, keeping open buffers.
    ///
    /// Used before re-reading configuration: what was discovered is no longer trustworthy, and the
    /// open buffers are, because the editor is still the authority on those.
    pub fn reset_index(&mut self) {
        self.format_style = FormatStyle::default();
        self.lint = LintRules::default();
        self.config_errors.clear();
        // What the editor said is deliberately kept. The client sent it once, at `initialize`, and it
        // is not going to send it again because a `composer.json` changed -- so clearing it here would
        // silently drop a person's settings partway through a session, which is the worst shape this
        // failure can take: it works until something unrelated happens.
        self.skeleton.adopt(ProjectSkeleton::new());
        self.autoload = AutoloadMap::default();
        self.frameworks.clear();
        self.configured_frameworks.clear();
        self.contributed_extensions.clear();
        self.contributed_config_extensions.clear();
        self.framework_sources.clear();
        self.path_rules = phrost_project::paths::PathRules::default();
        // The paths were about the *previous* discovery, and this is a reconfiguration -- where the
        // code lives may have moved, so keeping a path from before it would exempt a file from a
        // list that no longer describes the project.
        self.watched_since_discovery.clear();
        *self.discovering.get_mut() = false;
        // Cleared here and not by `adopt`, because this is the one caller that means it: the
        // configuration is about to be read again, and a framework that has been removed from the
        // project must not keep speaking for it.
        *self.skeleton.returns_mut() = phrost_index::ReturnContracts::default();
        // Both halves, because both were installed together and a list only one of them still holds
        // is how the open and the closed reading of one file come to disagree.
        self.skeleton
            .set_keyed_attributes(phrost_index::KeyedAttributes::default());
        self.open
            .set_keyed_attributes(phrost_index::KeyedAttributes::default());
    }

    pub fn file_id(&self, path: &Path) -> Option<FileId> {
        self.vfs.file_id(path)
    }

    pub fn path_of(&self, file: FileId) -> &Path {
        self.vfs.path(file)
    }

    /// The revision this server holds for an open buffer.
    ///
    /// For a task that went to sleep and has to ask whether it is still the latest. `None` for a
    /// file that is not open, which answers the same question the same way.
    pub fn document_version(&self, file: FileId) -> Option<DocumentVersion> {
        Some(self.vfs.open_document(file)?.version())
    }

    /// The version of whatever buffer is open at this path, PHP or configuration.
    ///
    /// By path rather than by `FileId`, because a configuration buffer has none: asked through the
    /// file table, a `services.yml` read as never open, so a quiet edit to one was never published.
    pub fn version_at(&self, path: &Path) -> Option<DocumentVersion> {
        match self.configs.get(path) {
            Some(held) => Some(held.version),
            None => self.document_version(self.vfs.file_id(path)?),
        }
    }

    /// The text of an open buffer, and its line map, for converting positions.
    pub fn document_text(&self, file: FileId) -> Option<(Arc<str>, LineIndex)> {
        let document = self.vfs.open_document(file)?;
        Some((
            document.source().shared_text(),
            LineIndex::new(document.source().text()),
        ))
    }

    /// An open template as the editor holds it, rather than as the PHP the index reads.
    pub fn written_template(&self, file: FileId) -> Option<Arc<str>> {
        self.written_templates.get(self.vfs.path(file)).cloned()
    }

    /// The text of an open buffer, for a request that rewrites it or draws on it: formatting, a
    /// code action, tokens and folds.
    ///
    /// Nothing for a template. What is held for one is the PHP its dialect made -- markup blanked,
    /// comments drawn around text, keywords written over directives -- which answers every question
    /// about an expression and is not what the person wrote: formatted, it would be rewritten
    /// into a PHP file, an import would land in the markup, and its tokens would paint a sentence
    /// as a comment.
    pub fn editable_text(&self, file: FileId) -> Option<(Arc<str>, LineIndex)> {
        if self.is_template(self.vfs.path(file)) {
            return None;
        }
        self.document_text(file)
    }

    /// The text of any indexed file, read from disk if it is not open.
    ///
    /// Used only to turn a target range into a client position. A single read of one file is not the
    /// request-time scanning the design rules out, and the alternative — keeping every indexed
    /// file's text resident — is exactly the weight being avoided.
    pub fn text_for_positions(&self, file: FileId) -> Option<(Arc<str>, LineIndex)> {
        if let Some(text) = self.document_text(file) {
            return Some(text);
        }
        let source = SourceText::read(self.vfs.path(file)).ok()?;
        Some((source.shared_text(), LineIndex::new(source.text())))
    }

    /// Where the index's heap went, by part.
    pub fn memory(&self) -> phrost_index::SkeletonMemory {
        self.skeleton.memory()
    }

    /// How many distinct names the index holds, and how much they cost.
    ///
    /// For the `index` command's shape report, which exists so that "names are interned, not
    /// duplicated" is a number somebody can look at rather than a claim in a document.
    pub fn interner_stats(&self) -> phrost_model::InternerStats {
        self.skeleton.interner().stats()
    }

    /// Distinct types across the whole project, which the same report watches for the same reason.
    pub fn type_count(&self) -> usize {
        self.skeleton.types().len()
    }

    /// How many keys each catalog holds, configuration and declarations together.
    ///
    /// In the shape report because a catalog is the one table here that a *framework* fills, so a
    /// wrong provider shows up as a zero rather than as a wrong answer somewhere else.
    pub fn catalog_sizes(&self) -> Vec<(&str, usize)> {
        self.skeleton.returns().catalog_sizes()
    }

    /// Bytes held by the buffers the editor has open.
    ///
    /// Its own figure rather than a row of the skeleton's breakdown, because it is the one number
    /// that is *supposed* to grow: the whole design is declarations for every file and text for the
    /// few being read.
    pub fn open_bytes(&self) -> usize {
        self.vfs
            .files()
            .filter_map(|file| self.vfs.open_document(file))
            .map(|document| document.source().text().len())
            .sum()
    }

    /// A view for a request about one file.
    pub fn view(&self, file: FileId) -> IndexView<'_> {
        IndexView::in_file(&self.skeleton, &self.open, file)
    }

    /// Every buffer the editor has open, with the revision this server holds.
    ///
    /// For re-checking them once the project is ready: a file opened while indexing was still
    /// running has no diagnostics, and nothing else would ever go back and produce them.
    pub fn open_documents(&self) -> Vec<(PathBuf, DocumentVersion)> {
        self.vfs
            .files()
            .filter_map(|file| {
                let document = self.vfs.open_document(file)?;
                Some((self.vfs.path(file).to_path_buf(), document.version()))
            })
            // Configuration buffers too: they are checked like PHP, and one opened while the
            // project was indexing is published by the sweep that walks this list when it ends.
            .chain(
                self.configs
                    .iter()
                    .map(|(path, held)| (path.clone(), held.version)),
            )
            .collect()
    }

    /// Files whose text may hold what `query` looks for, which is every file that could possibly
    /// reference it.
    ///
    /// Case-insensitively, because PHP resolves class and method names that way. Reading bytes is far
    /// cheaper than lowering them, and this is what turns a project-wide search from thousands of
    /// parses into a hundred.
    ///
    /// Every spelling in one pass rather than a pass each, because a target can have more than one
    /// -- a constructor is reached by `new Repo()` and by `parent::__construct()` -- and asking twice
    /// would read every file in the project twice for one question.
    ///
    /// The list comes from what the index holds, not from walking the project again. Those are the
    /// same files: a sweep can only answer about a file the index has a `FileId` for, so a file
    /// discovered here and unknown to the index was dropped a moment later anyway. Re-walking cost
    /// 477 ms of a search that had not yet looked at anything.
    ///
    /// The project's own files only. A reference from inside a dependency is not something anyone can
    /// act on, and reading 27 000 vendor files to report them would cost more than the answer is
    /// worth.
    pub fn reference_candidates(&self, query: &SweepQuery) -> Vec<PathBuf> {
        let words = WordNeedles::of(query.every_word());

        // What the index already knows, asked first. A file whose word set says the name is not
        // in it cannot hold a reference to it, so the only files left to read are the ones that
        // may -- six of 1 726 for a class name, where every one of them used to be read.
        let mut paths = Vec::new();
        for file in self.vfs.files() {
            let Some(skeleton) = self.skeleton.file(file) else {
                continue;
            };
            if !matches!(skeleton.origin(), SourceOrigin::User) {
                continue;
            }
            match skeleton.words() {
                // A file with no word set was never read into one -- a row built from an editor's
                // buffer, or an index written before this existed. It has learned nothing, so it
                // is read, which is what every file used to be.
                Some(set) if !set.may_hold_any(&words) => continue,
                _ => paths.push(self.vfs.path(file).to_path_buf()),
            }
        }

        read_matching(&paths, &candidates::TextNeedles::of(query))
    }

    /// Every place in one file that reaches `target`.
    ///
    /// One file rather than the project, so the caller decides how long to hold the lock and when to
    /// give the runtime a chance to cancel it. A sweep that answered for the whole project in one
    /// call held a read lock for half a second, which is a write lock a keystroke was waiting on --
    /// and it could not be interrupted, because a synchronous loop has no await for an abort to land
    /// on.
    ///
    /// A file the editor has open is matched from what the editor holds; the two differ exactly when
    /// someone is mid-edit, which is when this gets asked. Anything else is analysed into a scratch
    /// buffer set that this call drops, so nothing it touched outlives it -- not the index, not the
    /// open set, not the names it had to intern.
    pub fn references_in_file(&self, path: &Path, target: &ReferenceTarget) -> Vec<Reference> {
        let Some(file) = self.vfs.file_id(path) else {
            return Vec::new();
        };
        self.analysed(file, |view| collect(view, target, file))
            .unwrap_or_default()
    }

    /// Asks `question` of one file, analysed whether or not the editor has it open.
    ///
    /// The one mechanism for reaching a body that is not in a buffer, and the reason a project-wide
    /// question is affordable at all. A file the editor holds is answered from what the editor
    /// holds; the two differ exactly when someone is mid-edit, which is when this gets asked.
    /// Anything else is analysed into a scratch buffer set that this call drops, so nothing it
    /// touched outlives it -- not the index, not the open set, not the names it had to intern.
    ///
    /// Not a fallback in the sense §1.2 forbids. Nothing is asked of the index and found missing:
    /// bodies are open-file-only by design, so which of the two paths applies is decided by whether
    /// the file is open, before anything is looked up.
    ///
    /// `None` when the file cannot be read, which is a file deleted since it was indexed.
    pub fn analysed<T>(
        &self,
        file: FileId,
        question: impl FnOnce(&IndexView<'_>) -> T,
    ) -> Option<T> {
        if self.open.contains(file) {
            return Some(question(&self.view(file)));
        }

        let source = SourceText::read(self.vfs.path(file)).ok()?;
        Some(self.analysed_from(file, source.shared_text(), question))
    }

    /// The same, over text the caller already has -- what a pass over closed files hands in after
    /// reading each one once for its positions.
    pub fn analysed_from<T>(
        &self,
        file: FileId,
        text: Arc<str>,
        question: impl FnOnce(&IndexView<'_>) -> T,
    ) -> T {
        let mut scratch = OpenSet::new();
        // The project's list, so a file read for one request is lowered the same way the same file
        // would be if somebody had it open.
        scratch.set_keyed_attributes(self.skeleton.keyed_attributes().clone());
        // And a template read as PHP, for the same reason: the text was read from disk -- with the
        // files that render it beside it, which is what says what its variables are.
        let text = match self.skeleton.as_php(self.vfs.path(file), &text) {
            Some(php) => {
                self.open_renderers(file, &mut scratch);
                Arc::from(php)
            }
            None => text,
        };
        scratch.open(file, text);
        question(&IndexView::in_file(&self.skeleton, &scratch, file))
    }

    /// Whether every file the project owns is checked, or only the open ones.
    pub fn diagnoses_project(&self) -> bool {
        self.diagnose == Diagnose::Project
    }

    /// Every file the project owns that nobody has open, in path order.
    ///
    /// The open ones are the editor's: their findings come from the buffer, on the buffer's
    /// schedule, and a pass over the disk must not overwrite them with what the disk said.
    pub fn own_closed_files(&self) -> Vec<(PathBuf, FileId)> {
        let mut files = self
            .vfs
            .files()
            .filter(|&file| {
                self.vfs.origin(file) == SourceOrigin::User && !self.open.contains(file)
            })
            .map(|file| (self.vfs.path(file).to_path_buf(), file))
            .collect::<Vec<_>>();
        files.sort();
        files
    }

    /// The namespace the autoloader expects a file at `path` to declare, if any.
    ///
    /// The first of what the rules allow, which is the most specific -- see
    /// `ProjectSkeleton::permitted_namespaces` for why there can be several and why the first is
    /// the one a caller wanting one answer takes.
    pub fn permitted_namespace(&self, path: &Path) -> Option<String> {
        self.skeleton.permitted_namespaces(path).into_iter().next()
    }

    /// Whether a file is the project's own code rather than a dependency or a stub.
    ///
    /// Asked before a rename. A sweep only reads the project's files, so renaming a declaration that
    /// lives in `vendor/` would rewrite the uses it can see and leave the dependency's own -- and
    /// every other project sharing that dependency -- naming something that no longer exists.
    pub fn is_own_code(&self, file: FileId) -> bool {
        self.skeleton
            .file(file)
            .is_some_and(|skeleton| matches!(skeleton.origin(), SourceOrigin::User))
            && !self.is_a_dependency(self.vfs.path(file))
    }

    /// Whether a file the index holds no `FileId` for is the project's own: a configuration file or
    /// a template, opened in the editor and judged by where it lives.
    ///
    /// The same two halves [`Self::is_own_code`] asks of an indexed file -- the package manager's
    /// directory, and whatever a framework keeps elsewhere -- asked of the path alone.
    pub fn owns_path(&self, path: &std::path::Path) -> bool {
        self.layout
            .as_ref()
            .is_none_or(|layout| layout.source_origin(path) == SourceOrigin::User)
            && !self.is_a_dependency(path)
    }

    /// Whether a path is code the project reads and does not own.
    ///
    /// `SourceOrigin` is the package manager's directory and nothing more elaborate, which its own
    /// doc-comment insists on -- and a framework that keeps its code elsewhere is exactly what that
    /// cannot see. Drupal's `docroot/core` is 11 256 files outside `vendor/`, so every one of them
    /// counted as the project's own: not swept, because discovery honours the gitignore that hides
    /// them, and fully checked the moment one was opened. Which is the worst of the two -- findings
    /// about code somebody cannot change, in the file they opened to read.
    ///
    /// A byte prefix rather than `Path::starts_with`, for the reason §8 records about 849 roots: a
    /// component walk here is asked per file checked, and both paths are absolute and spelled by
    /// the same code.
    fn is_a_dependency(&self, path: &std::path::Path) -> bool {
        let path = path.as_os_str().as_encoded_bytes();
        self.dependency_paths.iter().any(|root| {
            let root = root.as_os_str().as_encoded_bytes();
            path.len() > root.len()
                && path.starts_with(root)
                && path[root.len()] == std::path::MAIN_SEPARATOR as u8
        })
    }

    /// A view for a request that is about no file in particular, like a project-wide search.
    pub fn project_view(&self) -> IndexView<'_> {
        IndexView::new(&self.skeleton, &self.open)
    }
}

/// Extensions a configuration rule can name. Not PHP, and read for what they *say* about PHP.
const CONFIG_EXTENSIONS: [&str; 2] = ["yml", "yaml"];

/// Whether a path is a template, which is neither PHP nor configuration and is read for a third
/// thing again: the member names it calls.
fn is_template_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            phrost_project::TEMPLATE_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

fn is_config_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| CONFIG_EXTENSIONS.contains(&extension))
}

/// What one walk of the project found, told apart by what it is for.
#[derive(Debug, Default)]
pub struct Discovered {
    /// Files to index, with whose code each is.
    pub php: Vec<(PathBuf, SourceOrigin)>,
    /// The project's own configuration, to read for the classes it names.
    pub configuration: Vec<PathBuf>,
    /// The project's own templates, to read for the members they call. See `install_template_words`.
    pub templates: Vec<PathBuf>,
}

/// An open configuration buffer: its text, and the revision the editor says it is.
#[derive(Debug)]
struct ConfigBuffer {
    text: Arc<str>,
    version: DocumentVersion,
}

/// What [`Workspace::set_document`] did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DocumentUpdate {
    Applied,
    /// Not PHP, but a configuration file some framework reads. Its text is held so a request in it
    /// can be answered and its keys checked against the catalogs; nothing was indexed.
    Held,
    /// Not a language this server reads, so nothing was held and nothing should be published.
    Ignored,
    /// A newer revision was already held, so nothing changed.
    Stale {
        held: DocumentVersion,
    },
}

/// What one watched change did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WatchedOutcome {
    /// Not a file this project indexes, or one whose open buffer is authoritative.
    Ignored,
    Reindexed,
    Removed,
    /// A framework's configuration or the project's own changed, so what is contributed may have.
    ConfigurationChanged,
    /// One of composer's files changed, so where the code is may have changed too -- or may be
    /// about to, if the file was the lock of a run still installing.
    ComposerChanged,
}

/// One place a declaration is used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reference {
    pub file: FileId,
    pub range: phrost_model::TextRange,
}

/// Answers `question` for every item, on every core the machine has, in order.
///
/// The sweep's own parallelism, shared by the two things it does: reading candidate files, and
/// analysing the ones that survive. Both are the same shape -- a pure question of one path, whose
/// answer does not depend on the others -- and on a reference sweep the second is now the larger:
/// parsing and lowering 124 candidates is 39.7 ms of a 42.3 ms answer, against 2.7 ms of actually
/// looking for the references in them.
///
/// Chunked and concatenated in order rather than work-stolen, because a client shows locations in
/// the order they arrive and an answer that reshuffles itself between two identical requests reads
/// as a bug.
pub(crate) fn in_parallel<I: Sync, T: Send>(
    items: &[I],
    question: impl Fn(&I) -> Vec<T> + Sync,
) -> Vec<T> {
    let workers = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .min(SCAN_WORKERS)
        .min(items.len().max(1));

    if workers <= 1 {
        return items.iter().flat_map(&question).collect();
    }

    let per = items.len().div_ceil(workers);
    let question = &question;
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(per)
            .filter_map(|chunk| {
                // `spawn_scoped` and not `scope.spawn`, which `deep_stacks` forbids outright --
                // and here it is not a formality: this one lowers PHP, which is the recursion the
                // check exists for.
                std::thread::Builder::new()
                    .name("phrost-sweep".to_owned())
                    .stack_size(phrost_model::DEEP_RECURSION_STACK)
                    .spawn_scoped(scope, move || {
                        chunk.iter().flat_map(question).collect::<Vec<_>>()
                    })
                    .ok()
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .flatten()
            .collect()
    })
}

/// The paths whose bytes may hold what `needles` look for.
///
/// The reading is the whole cost and the matching is none of it, which is not what it looks like:
/// scanning for a name is megabytes of `eq_ignore_ascii_case`, and stubbing the match out while
/// leaving the read in measured 26-29 ms against 29 ms with it. On this platform a file is its
/// `open()` -- 21 us against 2.5 us for a `stat` -- so what is left divides by cores and by
/// nothing else. Most files never reach here at all now; see [`WordSet`](phrost_index::WordSet).
fn read_matching(paths: &[PathBuf], needles: &candidates::TextNeedles) -> Vec<PathBuf> {
    in_parallel(paths, |path| {
        match std::fs::read(path).is_ok_and(|bytes| needles.found_in(&bytes)) {
            true => vec![path.clone()],
            false => Vec::new(),
        }
    })
}

/// Appends every reference one file holds.
fn collect(view: &IndexView<'_>, target: &ReferenceTarget, file: FileId) -> Vec<Reference> {
    phrost_analysis::references_in(view, target)
        .into_iter()
        .map(|range| Reference { file, range })
        .collect()
}

/// What identifies the build that derived an index.
///
/// The running executable's size and modification time. Not a version constant, because the thing
/// that invalidates a stored index is any change to the code that derives it -- and most of those
/// are not changes anybody would think to bump a version for. A rebuilt binary has a new
/// modification time, so a rebuilt server never reuses rows a different one wrote.
///
/// Zero when the executable cannot be found, which makes every start cold. That is the safe
/// direction: the alternative is reusing rows that may have been derived by other code.
///
/// Wrong only in the harmless direction. Copying the same binary elsewhere changes its timestamp and
/// costs one cold start; nothing makes two *different* builds look alike, because a build writes the
/// file.
fn build_stamp() -> u64 {
    let Ok(exe) = std::env::current_exe() else {
        return 0;
    };
    let Ok(stat) = std::fs::metadata(&exe) else {
        return 0;
    };
    let modified = stat
        .modified()
        .ok()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos() as u64);
    modified ^ stat.len().rotate_left(32)
}

/// Which files are checked and reported.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Diagnose {
    /// Every file the project owns, in the background once the index is ready and again when a file
    /// changes on disk -- and the open ones as they are typed in.
    #[default]
    Project,
    /// Only what the editor has open. What `[diagnostics] project = false` asks for.
    OpenOnly,
}
