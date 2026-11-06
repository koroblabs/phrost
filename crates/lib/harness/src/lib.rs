//! Builds an indexed project from a fixture, for tests above the index.
//!
//! Every layer above the index needs the same setup — parse a fixture, ingest its files, open one
//! of them — and every layer having its own copy of that setup is how a shared scaffolding file
//! turns into the thing everyone edits and nobody owns.
//!
//! It also owns the **open-versus-closed** check, in [`with_open_scopes`]. That invariant is the
//! one that has to be applied to every fixture rather than remembered per test, so it lives here
//! where it cannot be left out.

use std::collections::BTreeMap;

use phrost_index::{OpenSet, ProjectSkeleton};

/// Re-exported because a test that installs a framework contract has to name its shape, and the
/// layers above the store -- which is most of what these fixtures drive -- may not depend on it.
pub use phrost_index::{
    ConfigTarget, DeclaredInputsRule, DispatchRule, Forwarded, Place, PropertyNames, StatedKeyRule,
    StatedKind, StatedPropertiesRule, schema::WrittenSchemaRule,
};
use phrost_model::{FileId, LineIndex, SourceOrigin, TextRange};
use phrost_testkit::{FixtureSpec, MaterializedFixture};

/// An indexed project built from a fixture, plus the text needed to print positions.
///
/// The fixture is written to a temporary directory, not held only in memory. That is not extra
/// realism for its own sake: the index deliberately does not retain everything it can read back —
/// docblock text most of all — so a closed file's answers come from disk. A harness that skipped
/// the filesystem would make those answers silently unavailable, and only for closed files, which
/// is precisely the divergence the open-versus-closed check exists to catch.
#[derive(Debug)]
pub struct World {
    project: MaterializedFixture,
    skeleton: ProjectSkeleton,
    texts: BTreeMap<String, String>,
    ids: BTreeMap<String, FileId>,
}

impl World {
    /// Ingests every PHP file in the fixture.
    ///
    /// A path under `vendor/` is treated as a dependency and one under `stubs/` as a stub, because
    /// origin decides conflict ranking and a fixture needs a way to say which is which.
    pub fn build(spec: &FixtureSpec) -> Self {
        Self::build_with(spec, |_| {})
    }

    /// The same, with whatever the test knows about the project installed **before** any file is
    /// ingested.
    ///
    /// Which is the order the server follows, and it is load-bearing rather than tidy. Some facts
    /// are derived *at* ingest and never re-read: the extension points a file's calls declare are
    /// found while its bodies are still in hand, so a file ingested before the rule that says which
    /// calls dispatch declares nothing at all -- and the only symptom is a feature that does not
    /// work, in a test that reads as though it set the rule up correctly.
    ///
    /// The world handed to `state` has no files yet, so it can be asked about the project and not
    /// about its contents.
    pub fn build_with(spec: &FixtureSpec, state: impl FnOnce(&mut Self)) -> Self {
        let project = MaterializedFixture::new(spec.clone());
        let mut skeleton = ProjectSkeleton::new();
        skeleton.set_root(project.root());

        let mut world = Self {
            project,
            skeleton,
            texts: BTreeMap::new(),
            ids: BTreeMap::new(),
        };
        state(&mut world);

        for (index, file) in spec
            .files()
            .iter()
            .filter(|file| file.path().ends_with(".php"))
            .enumerate()
        {
            let id = FileId::from_raw(u32::try_from(index).expect("fixture file count fits u32"));
            let path = world.project.path(file.path());
            world
                .skeleton
                .ingest(id, &path, origin_of(file.path()), file.text());
            world
                .texts
                .insert(file.path().to_owned(), file.text().to_owned());
            world.ids.insert(file.path().to_owned(), id);
        }
        world.skeleton.mark_ready();

        world
    }

    /// The project's framework knowledge, for a test to state before it asks anything.
    ///
    /// The same handle the server's startup writes through, rather than one method per kind of
    /// contract: a fixture has no framework to detect, and mirroring the table's API here would mean
    /// editing the harness every time a provider learns to contribute something new.
    pub fn contracts_mut(&mut self) -> &mut phrost_index::ReturnContracts {
        self.skeleton.returns_mut()
    }

    /// Interns the shape every complete property table describes, the way the server does it once
    /// the framework's tables are in. A test that states tables and forgets this gets a property
    /// answering and the whole-table read silent.
    pub fn intern_property_shapes(&mut self) {
        self.skeleton.intern_property_shapes();
    }

    /// The temporary directory the fixture was written to.
    /// The house style this project's code is written in, as a framework would have named it.
    /// The attributes whose literals lowering keeps, which a test states the way the server does:
    /// before the first file is ingested, since a file lowered without them declares nothing.
    ///
    /// Its own method rather than through [`Self::contracts_mut`] because it is not a contract -- it
    /// is the gate in front of lowering, and the two live on opposite sides of the parser boundary.
    /// Built here rather than taken as a type, so a crate that may not depend on the index -- which
    /// is every feature crate -- can still state it.
    /// The property defaults lowering keeps, stated the way the server derives them from a
    /// stated-properties rule -- before the first file is ingested, for the reason
    /// [`Self::set_keyed_attributes`] gives.
    pub fn set_kept_defaults(&mut self, properties: &[&str], attributes: &[&str]) {
        self.skeleton.set_keyed_attributes(
            phrost_index::KeyedAttributes::default()
                .keeping_defaults_of(properties.iter().copied())
                .keeping_lists_of(attributes.iter().copied()),
        );
    }

    /// The methods whose `return X::class;` makes `X` a mixin of the class, stated before the
    /// files for the reason [`Self::set_kept_defaults`] is.
    pub fn set_mixin_methods(&mut self, methods: &[&str]) {
        self.skeleton.set_keyed_attributes(
            phrost_index::KeyedAttributes::default().mixing_in_returns_of(methods.iter().copied()),
        );
    }

    /// The objects named methods build to declare entries, each `(method, class, catalog)`, stated
    /// before the files for the reason [`Self::set_kept_defaults`] is: lowering keeps only what it
    /// was told to.
    pub fn set_constructed_entries(&mut self, entries: &[(&str, &str, &str)]) {
        for &(method, class, catalog) in entries {
            self.skeleton
                .returns_mut()
                .bind_constructed_entry(method, class, catalog);
        }
        self.skeleton.set_keyed_attributes(
            phrost_index::KeyedAttributes::default().keeping_constructions(
                entries.iter().map(|(method, ..)| *method),
                entries.iter().map(|(_, class, _)| *class),
            ),
        );
    }

    pub fn set_keyed_attributes(&mut self, attributes: &[&str], entering: &[&str]) {
        self.skeleton.set_keyed_attributes(
            phrost_index::KeyedAttributes::of(attributes.iter().copied())
                .entering(entering.iter().copied()),
        );
    }

    /// A declaration whose body states the properties of its kind: `#[attribute('kind')]` on a class
    /// whose `method` writes `$t['key'] = Builder::create('name')`, each name meaning one of
    /// `kinds`. The first positional argument is the kind, and the attribute is keyed too, since a
    /// table is read at ingest and lowering keeps no literal it was not told to.
    ///
    /// Here for a crate that may not name the index's rule type, for [`Self::set_keyed_attributes`]'s
    /// reason; call it inside [`Self::build_with`], before the files are read.
    pub fn add_declared_table(
        &mut self,
        attribute: &str,
        method: &str,
        builders: &[&str],
        kinds: &[(&str, phrost_model::BuiltinTy)],
        catalog: &str,
    ) {
        self.set_keyed_attributes(&[attribute], &[]);
        self.skeleton
            .returns_mut()
            .add_declared_table(phrost_index::DeclaredTableRule {
                attribute: attribute.into(),
                argument: phrost_index::AttributeArgument::At(0),
                method: method.into(),
                builders: builders.iter().map(|&builder| builder.into()).collect(),
                kinds: kinds
                    .iter()
                    .map(|&(name, holds)| (name.into(), holds))
                    .collect(),
                catalog: catalog.into(),
            });
    }

    /// A global name a framework's loader gives `class` at start. See the seam's `ClassAlias`.
    pub fn add_class_alias(&mut self, alias: &str, class: &str) {
        self.skeleton.add_class_alias(alias, class);
    }

    /// A `@mixin` a framework says `class` behaves as though it wrote -- `mixin<static>` when
    /// `with_static`. See the framework seam's `ImpliedMixin`.
    pub fn add_implied_mixin(&mut self, class: &str, mixin: &str, with_static: bool) {
        self.skeleton.add_implied_mixin(class, mixin, with_static);
    }

    /// The class `class`'s `@method` tags were generated from. See the framework seam's
    /// `MethodsTaggedFrom`.
    pub fn add_tag_source(&mut self, class: &str, source: &str) {
        self.skeleton.add_tag_source(class, source);
    }

    /// The spelling that makes a string name a method of the class it is written in.
    ///
    /// Drupal's `'::districtCallback'`. Stated rather than detected, for the reason every other
    /// framework fact here is: a fixture has no framework to find.
    pub fn set_own_method_prefixes(&mut self, prefixes: &[&str]) {
        self.skeleton
            .add_own_method_prefixes(prefixes.iter().map(|prefix| (*prefix).into()));
    }

    /// Calls on a test double whose string argument names a method of what it stands in for: one
    /// rule, `stand_in::method(at)`, looking through the builder calls in `through`.
    ///
    /// PHPUnit's `method()` after `expects()`. Stated rather than detected, for the reason every
    /// other framework fact here is.
    pub fn set_stand_in_method(&mut self, stand_in: &str, method: &str, at: u16, through: &[&str]) {
        self.skeleton
            .add_stand_in_methods([phrost_index::StandInMethod {
                stand_in: stand_in.into(),
                method: method.into(),
                at,
                through: through.iter().map(|call| (*call).into()).collect(),
            }]);
    }

    /// A call whose first argument names the key of an element the enclosing class builds, with
    /// Drupal's spelling of what an element is: `#`-prefixed properties, `#type`, `#parents`.
    pub fn set_built_key(&mut self, class: &str, method: &str) {
        self.skeleton.add_built_keys([phrost_index::BuiltKey {
            class: class.into(),
            method: method.into(),
            property: "#".into(),
            described_by: "#type".into(),
            filed_under: "#parents".into(),
        }]);
    }

    /// A call naming the class the closures a callee binds run as, the way a framework states it.
    pub fn set_configured_this(
        &mut self,
        declared: &str,
        functions: &[&str],
        methods: &[&str],
        within: &str,
    ) {
        self.skeleton
            .returns_mut()
            .bind_configured_this(phrost_index::ConfiguredThisRule {
                declared: declared.into(),
                functions: functions.iter().map(|&name| name.into()).collect(),
                methods: methods.iter().map(|&name| name.into()).collect(),
                within: within.into(),
            });
    }

    /// The namespace each source directory stands for, as composer's generated map would say.
    ///
    /// Stated rather than read, because a fixture has no `vendor/composer/autoload_psr4.php` -- the
    /// map comes from what composer *dumped*, not from a `composer.json`, so a project without a
    /// vendor directory has no PSR-4 rules at all. Paths are relative to the fixture's own root.
    pub fn set_namespace_rules(&mut self, rules: &[(&str, &str)]) {
        let root = self.project.root().to_path_buf();
        self.skeleton.set_namespace_rules(
            rules
                .iter()
                .map(|(prefix, dir)| ((*prefix).into(), root.join(dir)))
                .collect(),
        );
    }

    /// What the project's own templates write, as the words the reader compares against.
    ///
    /// Stated rather than walked, for the reason every other fact here is: a fixture has no
    /// `templates/` directory, and the words are read from files the index never holds. The getter
    /// spellings are expanded here exactly as the workspace expands them, so a test says
    /// `{{ trade.status }}` and gets `getstatus` with it.
    pub fn set_template_words(&mut self, written: &[&str]) {
        let mut words: std::collections::HashSet<Box<str>> = std::collections::HashSet::new();
        for word in written {
            let word = word.to_ascii_lowercase();
            for prefix in ["get", "is", "has"] {
                words.insert(format!("{prefix}{word}").into_boxed_str());
            }
            words.insert(word.into_boxed_str());
        }
        self.skeleton.set_template_words(words);
    }

    /// Records the names a template renders others by, as the server records them from the
    /// project's templates when it starts.
    pub fn set_twig_includes(&mut self, template: &std::path::Path, names: &[&str]) {
        self.skeleton
            .set_twig_includes(template, names.iter().map(|&name| name.into()).collect());
    }

    pub fn set_house_style(&mut self, style: &str) {
        self.skeleton.set_house_style(Some(style));
    }

    pub fn root(&self) -> &std::path::Path {
        self.project.root()
    }

    pub fn parse(fixture: &str) -> Self {
        Self::build(&FixtureSpec::parse(fixture))
    }

    pub fn skeleton(&self) -> &ProjectSkeleton {
        &self.skeleton
    }

    pub fn file_id(&self, path: &str) -> FileId {
        self.ids[path]
    }

    /// The first file in path order, for queries that need a current file but no cursor.
    pub fn any_file(&self) -> Option<FileId> {
        self.ids.values().next().copied()
    }

    pub fn path_of(&self, file: FileId) -> &str {
        self.ids
            .iter()
            .find(|(_, id)| **id == file)
            .map(|(path, _)| path.as_str())
            .expect("file was registered")
    }

    pub fn text_of(&self, file: FileId) -> &str {
        &self.texts[self.path_of(file)]
    }

    /// Opens the buffers a scope calls for.
    pub fn open(&self, scope: OpenScope, cursor: Option<FileId>) -> OpenSet {
        let mut open = OpenSet::new();
        // What lowering keeps, as the server hands both halves of the index the same list: a buffer
        // lowered without it holds none of the literals and defaults a framework reads from it.
        open.set_keyed_attributes(self.skeleton.keyed_attributes().clone());

        match scope {
            OpenScope::CursorFileOnly => {
                if let Some(file) = cursor {
                    open.open(file, self.text_of(file).to_owned());
                }
            }
            OpenScope::EveryFile => {
                for (path, &file) in &self.ids {
                    open.open(file, self.texts[path].clone());
                }
            }
        }

        open
    }

    /// One-based `line:col-line:col`, for snapshot rows.
    pub fn render_range(&self, file: FileId, range: TextRange) -> String {
        let (start, end) = LineIndex::new(self.text_of(file)).range_line_cols(range);
        format!(
            "{}:{}-{}:{}",
            start.line + 1,
            start.col + 1,
            end.line + 1,
            end.col + 1
        )
    }
}

/// Which buffers are open while a query runs.
#[derive(Clone, Copy, Debug)]
pub enum OpenScope {
    /// Only the file the cursor is in — the minimum a position query needs.
    CursorFileOnly,
    /// Every file in the fixture, so that consulting another open buffer would change an answer.
    EveryFile,
}

/// Runs `answer` under both open scopes and fails if they disagree.
///
/// The invariant: a cross-file answer must not depend on which buffers happen to be open. Today it
/// holds structurally — the view can reach only the current file — so this passes by construction.
/// Its job is to fail the first time someone makes an answer quietly better for open files, which
/// is the bug class that made the previous attempt feel unpredictable.
pub fn with_open_scopes(
    world: &World,
    cursor: Option<FileId>,
    answer: impl Fn(&OpenSet) -> String,
) -> String {
    let with_one = answer(&world.open(OpenScope::CursorFileOnly, cursor));
    let with_all = answer(&world.open(OpenScope::EveryFile, cursor));

    assert_eq!(
        with_one, with_all,
        "the answer changed when other files were opened; a cross-file answer must not depend on \
         which buffers happen to be open"
    );

    with_one
}

fn origin_of(path: &str) -> SourceOrigin {
    if path.starts_with("vendor/") {
        SourceOrigin::Vendor
    } else if path.starts_with("stubs/") {
        SourceOrigin::Stub
    } else {
        SourceOrigin::User
    }
}
