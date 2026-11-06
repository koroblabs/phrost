//! Index tests, including the invariant everything else leans on.
//!
//! Two harnesses. [`check_index`] snapshots the whole skeleton, which is the readable description
//! of what indexing produced. [`check_incremental`] additionally proves that applying edits and
//! indexing from scratch reach the *same* skeleton, byte for byte.
//!
//! That second one is the important one. Invalidation bugs are the worst class in a language
//! server — they surface hours later as a stale answer with no way to trace it back — and they are
//! invisible to any test that only ever builds an index once.

mod a_branch_a_callback_returns;
mod a_branch_holding_a_bounded_template;
mod a_branch_naming_the_keys;
mod a_branch_picked_by_a_template;
mod a_class_a_framework_runs;
mod a_constant_table_read_by_key;
mod a_container_of_the_value_passed;
mod a_left_out_argument;
mod a_return_naming_its_own_template;
mod a_table_a_declaration_states;
mod a_table_migrations_write;
mod a_trait_followed_to_its_users;
mod adopting;
mod an_array_rendering_a_template;
mod an_entry_a_construction_declares;
mod an_object_shape;
mod annotation_keys;
mod applied_contracts;
mod cache;
mod cache_pruning;
mod catalog;
mod conditional_ranking;
mod configured_this;
mod derived_keys;
mod discovered_namespaces;
mod dispatched_names;
mod edit_fuzz;
mod flattened_memo;
mod ignored_ranking;
mod incremental;
mod macros_a_body_registers;
mod member_types;
mod members;
mod memory;
mod parallel_ingest;
mod positional_attribute_keys;
mod receiver_out;
mod row_sizes;
mod scaling;
mod state;
mod stored_notes;
mod warm_reuse;

use std::{collections::BTreeMap, path::PathBuf};

use expect_test::Expect;
use phrost_model::{FileId, LineIndex, SourceOrigin};
use phrost_testkit::FixtureSpec;

use crate::{ProjectSkeleton, dump_skeleton};

/// One edit applied to an already-indexed project.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Step {
    /// Create the file, or replace its contents.
    Write {
        path: &'static str,
        text: &'static str,
    },
    /// Remove the file, as a delete on disk would.
    Delete { path: &'static str },
}

impl Step {
    pub(crate) fn write(path: &'static str, text: &'static str) -> Self {
        Self::Write { path, text }
    }

    pub(crate) fn delete(path: &'static str) -> Self {
        Self::Delete { path }
    }
}

/// Indexes the fixture and snapshots the resulting skeleton.
pub(crate) fn check_index(fixture: &str, expect: Expect) {
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    expect.assert_eq(&built.render());
}

/// Indexes the fixture, applies `steps`, and proves the result equals indexing the final state
/// from scratch.
///
/// The from-scratch build deliberately assigns file ids in the **opposite** order, so anything that
/// leaked a dependency on discovery order -- a conflict ranked by `FileId`, a map iterated in hash
/// order -- shows up as a mismatch here rather than as an occasional wrong answer months later.
pub(crate) fn check_incremental(fixture: &str, steps: &[Step], expect: Expect) {
    let mut files = fixture_files(fixture);
    let mut built = build(&files, Order::ByPath);

    for step in steps {
        match *step {
            Step::Write { path, text } => {
                let text = normalize(text);
                let id = built.ids.assign(path);
                built
                    .skeleton
                    .ingest(id, &fixture_path(path), origin_of(path), &text);
                built.texts.insert(path.to_owned(), text.clone());
                files.insert(path.to_owned(), text);
            }
            Step::Delete { path } => {
                // Ids are never reused, exactly as the real VFS behaves: a stale index row must not
                // be able to start pointing at an unrelated file.
                built.skeleton.remove(built.ids.assign(path));
                built.texts.remove(path);
                files.remove(path);
            }
        }
    }
    built.skeleton.mark_ready();

    let fresh = build(&files, Order::ReversedByPath);
    let applied = built.render();
    assert_eq!(
        applied,
        fresh.render(),
        "applying edits produced a different index than building from scratch"
    );

    // Stamps are left out of the dump to keep it readable, so check them here: a stamp that differs
    // for identical content means content hashing lost track of a revision.
    for path in files.keys() {
        let left = built.stamp(path).expect("file present after edits");
        let right = fresh.stamp(path).expect("file present in the fresh index");
        assert_eq!(left, right, "stamp differs for {path}");
    }

    expect.assert_eq(&applied);
}

#[derive(Clone, Copy)]
pub(crate) enum Order {
    ByPath,
    ReversedByPath,
}

/// Append-only path-to-id allocator, mirroring how the real VFS hands out file ids.
#[derive(Debug, Default)]
struct FileIds {
    by_path: BTreeMap<String, FileId>,
    next: u32,
}

impl FileIds {
    /// Every id assigned, in path order.
    pub(crate) fn all(&self) -> impl Iterator<Item = FileId> + '_ {
        self.by_path.values().copied()
    }

    fn assign(&mut self, path: &str) -> FileId {
        if let Some(&existing) = self.by_path.get(path) {
            return existing;
        }

        let id = FileId::from_raw(self.next);
        self.next += 1;
        self.by_path.insert(path.to_owned(), id);
        id
    }

    fn get(&self, path: &str) -> Option<FileId> {
        self.by_path.get(path).copied()
    }
}

/// One built index, plus what the dump needs to print positions.
pub(crate) struct Built {
    pub(crate) skeleton: ProjectSkeleton,
    ids: FileIds,
    texts: BTreeMap<String, String>,
}

impl Built {
    fn render(&self) -> String {
        dump_skeleton(&self.skeleton, |entry| {
            self.texts
                .get(self.skeleton.display_path(entry))
                .map(|text| LineIndex::new(text))
        })
    }

    fn stamp(&self, path: &str) -> Option<phrost_vfs::FileStamp> {
        Some(self.skeleton.file(self.ids.get(path)?)?.stamp())
    }

    pub(crate) fn file_id(&self, path: &str) -> Option<FileId> {
        self.ids.get(path)
    }
}

/// The root these tests pretend their files live under.
///
/// Absolute because the index stores the path it would read a docblock back from, and a relative one
/// only works when whatever runs happens to sit in the right directory. Nothing here reads from
/// disk, but storing a path that could not be read is how a server ends up with no documentation for
/// every closed file.
const FIXTURE_ROOT: &str = "/fixture";

fn fixture_path(path: &str) -> PathBuf {
    PathBuf::from(FIXTURE_ROOT).join(path)
}

/// A skeleton with one rule of every kind a framework can contribute, and no files.
///
/// Extracted so a second way of taking files in can be measured against the first with the same
/// configuration -- a comparison where one side had a rule the other did not would prove nothing.
pub(crate) fn configured(root: &str) -> ProjectSkeleton {
    let mut skeleton = ProjectSkeleton::new();
    skeleton.set_root(root);
    skeleton
        .returns_mut()
        .name_keys_with("Plugin", "id", "plugin");
    skeleton
        .returns_mut()
        .position_keys_with("Element", 0, "element");
    // And the list lowering needs to keep that positional literal at all -- and to read the older
    // annotation spelling of either rule -- installed the way the server installs it: every
    // attribute a rule names, before the first file is ingested.
    // And one rule of the derived kind, for the same reason and with the same neutral names: a key
    // spelled out of two of a declaration's own arguments is claimed and withdrawn like any other,
    // and a withdrawal that forgot it is exactly what the incremental dump exists to catch.
    skeleton.returns_mut().derive_keys_with(
        "Plugin",
        "links",
        "id",
        "page.{name}.{key}",
        &[("x:", ""), ("-", "_")],
        "page",
    );
    skeleton.set_keyed_attributes(
        crate::KeyedAttributes::of(["Element", "Plugin"])
            .entering(["links"])
            .keeping_defaults_of(["table", "model"])
            .mixing_in_returns_of(["getFacadeAccessor"])
            .keeping_constructions(["getFilters"], ["Twig\\TwigFilter"]),
    );
    // And one rule of the third kind -- a key carried in a declaration's own *name* -- for the
    // reason the two above give, and because it is the kind that was missing from `adopt`: a warm
    // start held every key an attribute named and none that a name carried, and no test at this
    // layer could see it because no test installed the rule. The prefix catalog is seeded here too,
    // since a name only carries a key if something says the prefix is one.
    // And one declaration whose *body* states a table, for the reason the dispatchers below give:
    // a fact read out of a body is one the cache has to carry, and a rule no test installs is a
    // codec field no round trip can see. Neutral names again.
    // And a call rendering a template by name, which only a body says -- so the codec has to carry
    // it, and the round trip has to see it carried.
    skeleton
        .returns_mut()
        .bind_rendered_template("render", 0, 1, false);
    skeleton
        .returns_mut()
        .bind_rendered_template("partial", 0, 1, true);
    // And an object a named method builds to declare an entry, which a dependency's body says and
    // so the codec has to carry.
    skeleton
        .returns_mut()
        .bind_constructed_entry("getFilters", "Twig\\TwigFilter", "filter");
    // And an array naming the template it renders, which a body says too and is filed beside the
    // calls.
    skeleton
        .returns_mut()
        .bind_render_array(crate::RenderArrayRule {
            key: "#template".into(),
            catalog: "template".into(),
            prefix: "#".into(),
            own: ["#template".into(), "#cache".into()].into(),
            file_name: None,
        });
    skeleton
        .returns_mut()
        .bind_shared_template_data("App\\Shared", "share", Some("shared"));
    // And a call naming the class a callee's bound closures run as, for the same reason.
    skeleton
        .returns_mut()
        .bind_configured_this(crate::ConfiguredThisRule {
            declared: "App\\Base".into(),
            functions: Box::new(["configure".into()]),
            methods: Box::new(["with".into()]),
            within: "in".into(),
        });
    skeleton
        .returns_mut()
        .add_declared_table(crate::DeclaredTableRule {
            attribute: "Element".into(),
            argument: crate::AttributeArgument::At(0),
            method: "shapeOf".into(),
            builders: vec!["Slot".into()],
            kinds: vec![
                ("text".into(), phrost_model::BuiltinTy::String),
                ("number".into(), phrost_model::BuiltinTy::Int),
            ],
            catalog: "element".into(),
        });
    // And a schema a migration's calls write, with the table a class names: a body and a kept
    // default, which only the codec carries to a warm start. Neutral names and a two-word builder.
    skeleton
        .returns_mut()
        .bind_written_schema(crate::schema::WrittenSchemaRule {
            facade: "Db\\Schema".into(),
            create: vec!["create".into()],
            alter: vec!["table".into()],
            drop: vec!["drop".into()],
            rename: vec!["rename".into()],
            runs: "up".into(),
            directory: "migrations".into(),
            columns: neutral_columns,
            model: "Db\\Model".into(),
            table_property: "table".into(),
            default_table: |short| format!("{}s", short.to_ascii_lowercase()),
        });
    skeleton
        .returns_mut()
        .bind_macro_registrar("Db\\Macroable", "macro");
    skeleton
        .returns_mut()
        .derive_names_with("package", "listener_", "signal");
    skeleton
        .returns_mut()
        .catalog_entry("package", "shop", None);
    // And one dispatching method of each shape, for the same reason: a fact derived from a body is
    // one every test here should be exercising, not only the ones that remember to ask for it. The
    // suffix and the list are the two things a rule can vary, and both are in the seam because one
    // real framework's `alter($type)` runs `hook_{$type}_alter` while its `invokeAll($hook)` does
    // not. Neutral names, like the attribute rules above.
    skeleton.returns_mut().add_dispatcher(
        "publish",
        crate::DispatchRule {
            class: "A\\Hub".into(),
            argument: 0,
            several: false,
            names: "on_".into(),
            suffix: "".into(),
            catalog: "event".into(),
        },
    );
    skeleton.returns_mut().add_dispatcher(
        "announce",
        crate::DispatchRule {
            class: "A\\Hub".into(),
            argument: 1,
            several: true,
            names: "on_".into(),
            suffix: "_done".into(),
            catalog: "event".into(),
        },
    );
    skeleton
}

/// What [`configured`]'s schema builder makes of a call: `text('x')` and `number('x')` add a column,
/// `->nullable()` lets it be null, `forget('x', ...)` drops columns and `move('x', 'y')` renames one.
fn neutral_columns(
    call: &phrost_model::schema::BuilderCall,
) -> Vec<phrost_model::schema::ColumnChange> {
    use phrost_model::schema::ColumnChange;
    let first = call.arguments.first().and_then(|written| written.first());
    let nullable = call.then.iter().any(|(method, _)| method == "nullable");
    let holds = match call.method.as_str() {
        "text" => phrost_model::BuiltinTy::String,
        "number" => phrost_model::BuiltinTy::Int,
        "forget" => {
            return call
                .arguments
                .iter()
                .flatten()
                .map(|name| ColumnChange::Dropped(name.clone()))
                .collect();
        }
        "move" => {
            let to = call.arguments.get(1).and_then(|written| written.first());
            return first
                .zip(to)
                .map(|(from, to)| ColumnChange::Renamed {
                    from: from.clone(),
                    to: to.clone(),
                })
                .into_iter()
                .collect();
        }
        _ => return Vec::new(),
    };
    first
        .map(|name| ColumnChange::Defined {
            name: name.clone(),
            holds,
            nullable,
            constrained: call.then.iter().any(|(method, _)| method == "constrained"),
        })
        .into_iter()
        .collect()
}

pub(crate) fn build(files: &BTreeMap<String, String>, order: Order) -> Built {
    let mut paths = files.keys().cloned().collect::<Vec<_>>();
    if matches!(order, Order::ReversedByPath) {
        paths.reverse();
    }

    let skeleton = configured(FIXTURE_ROOT);
    // One attribute rule of each spelling, so the declared-key half of ingest is exercised by every
    // test here rather than only by the ones that remember to install it. Neutral names: what a
    // catalog means is a framework's business and no test at this layer needs to know.
    let mut built = Built {
        skeleton,
        ids: FileIds::default(),
        texts: files.clone(),
    };

    for path in &paths {
        let id = built.ids.assign(path);
        built
            .skeleton
            .ingest(id, &fixture_path(path), origin_of(path), &files[path]);
    }
    built.skeleton.mark_ready();

    built
}

/// Anything under `vendor/` is a dependency, which is what decides conflict ranking.
fn origin_of(path: &str) -> SourceOrigin {
    if path.starts_with("vendor/") {
        SourceOrigin::Vendor
    } else if path.starts_with("stubs/") {
        SourceOrigin::Stub
    } else {
        SourceOrigin::User
    }
}

pub(crate) fn fixture_files(fixture: &str) -> BTreeMap<String, String> {
    FixtureSpec::parse(fixture)
        .files()
        .iter()
        .filter(|file| file.path().ends_with(".php"))
        .map(|file| (file.path().to_owned(), file.text().to_owned()))
        .collect()
}

/// Applies the same leading-newline trim the fixture format uses, so step text can be written the
/// same way fixture text is.
fn normalize(text: &str) -> String {
    text.strip_prefix('\n').unwrap_or(text).to_owned()
}
