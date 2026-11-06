//! Def-map snapshot tests.
//!
//! The second layer of the per-layer story. When a goto lands in the wrong place, the question
//! "did the name resolve to the right candidates?" is answered here — pointing the same fixture at
//! this harness instead of at the whole server.

mod a_built_scope_holds_no_spare_capacity;
mod resolution;
mod scopes;

use std::fmt::Write as _;

use expect_test::Expect;
use phrost_hir::{ImportKind, NameAnchor};
use phrost_model::{FileId, Interner, LineIndex};
use phrost_syntax::ParseSession;
use phrost_testkit::FixtureSpec;

use crate::{FileScope, render_file_scope};

/// Snapshots the scope of every PHP file in the fixture: its namespaces, imports, and the
/// fully-qualified names it declares.
pub(crate) fn check_scope(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut interner = Interner::new();
    let mut out = String::new();

    let files = php_files(&spec);
    let multiple = files.len() > 1;

    for (index, file) in files {
        let session = ParseSession::new();
        let parsed = session.parse(file.text());
        let lowered = crate::tests::lower(&mut interner, index, &parsed);
        let scope = FileScope::build(&mut interner, &lowered);

        if multiple {
            let _ = writeln!(out, "// {}", file.path());
        }
        out.push_str(&render_file_scope(&scope, &LineIndex::new(file.text())));
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&format!("{}\n", out.trim_end()));
}

/// Resolves the written name under each marker, as a class-like lookup.
///
/// Markers must sit inside a type position, which is where lowering records written names today.
pub(crate) fn check_resolution(fixture: &str, markers: &[&str], expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut interner = Interner::new();
    let mut out = String::new();

    // Resolution needs one file's scope; a fixture for it declares exactly one PHP file.
    let (index, file) = php_files(&spec)
        .into_iter()
        .next()
        .expect("resolution fixtures declare one PHP file");
    let session = ParseSession::new();
    let parsed = session.parse(file.text());
    let hir = crate::tests::lower(&mut interner, index, &parsed);
    let scope = FileScope::build(&mut interner, &hir);

    for name in markers {
        let marker = spec.markers().position(name);
        let (_, name_ref) = hir
            .name_refs()
            .find(|(_, data)| data.range.contains_inclusive(marker.offset))
            .unwrap_or_else(|| {
                panic!("no written name at marker `{name}`; markers must sit in a type position")
            });

        let resolution = scope.scope_at(marker.offset).resolve(
            name_ref.path.as_str(),
            name_ref.anchor,
            ImportKind::ClassLike,
        );

        let _ = writeln!(out, "{name}: {} ({:?})", name_ref.path, name_ref.anchor);
        render_resolution(&mut out, &resolution);
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&format!("{}\n", out.trim_end()));
}

/// Resolves literal names in the scope at `at`, across all three symbol tables.
///
/// Needed because function and constant references live in expression bodies, which are not
/// lowered yet — but the rules for those tables differ from the class-like one in exactly the ways
/// that cause wrong answers, so they need covering now.
pub(crate) fn check_lookup(
    fixture: &str,
    at: &str,
    lookups: &[(&str, NameAnchor, ImportKind)],
    expect: Expect,
) {
    let spec = FixtureSpec::parse(fixture);
    let mut interner = Interner::new();
    let (index, file) = php_files(&spec)
        .into_iter()
        .next()
        .expect("lookup fixtures declare one PHP file");

    let session = ParseSession::new();
    let parsed = session.parse(file.text());
    let hir = crate::tests::lower(&mut interner, index, &parsed);
    let scope = FileScope::build(&mut interner, &hir);
    let namespace = scope.scope_at(spec.markers().position(at).offset);

    let mut out = String::new();
    for (path, anchor, table) in lookups {
        let _ = writeln!(
            out,
            "{path} ({:?}, {})",
            anchor,
            match table {
                ImportKind::ClassLike => "class",
                ImportKind::Function => "function",
                ImportKind::Constant => "const",
            }
        );
        render_resolution(&mut out, &namespace.resolve(path, *anchor, *table));
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&format!("{}\n", out.trim_end()));
}

fn render_resolution(out: &mut String, resolution: &crate::NameResolution) {
    for candidate in resolution.candidates() {
        let _ = writeln!(out, "  {} via {:?}", candidate.fqn(), candidate.origin());
    }
}

fn php_files(spec: &FixtureSpec) -> Vec<(u32, &phrost_testkit::FixtureFile)> {
    spec.files()
        .iter()
        .enumerate()
        .filter(|(_, file)| file.path().ends_with(".php"))
        .map(|(index, file)| {
            (
                u32::try_from(index).expect("fixture file count fits in u32"),
                file,
            )
        })
        .collect()
}

fn lower(
    interner: &mut Interner,
    index: u32,
    parsed: &phrost_syntax::ParsedFile<'_>,
) -> phrost_hir::FileHir {
    let lowered = phrost_lower::lower_file(
        interner,
        FileId::from_raw(index),
        parsed,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "def-map fixtures should parse cleanly, got {:?}",
        lowered.errors
    );
    lowered.hir
}
