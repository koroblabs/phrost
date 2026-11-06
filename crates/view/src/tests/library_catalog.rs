//! The array key that names a Drupal asset library.
//!
//! `'#attached' => ['library' => ['a/b']]`. A library name is written *inside a list* under the
//! key, not as the key's own value the way `#type` is -- so this exercises the one extension
//! [`phrost_body::keyed_elements`] needed: a flat list of bare strings under a bound key is exploded
//! into one entry per item, and everything downstream of that (this catalog included) reads it
//! exactly the way [`element_catalog`](super::element_catalog) reads `#type`.
//!
//! Filled with `catalog_place` rather than `catalog_entry`, unlike the element catalog: a library is
//! declared in a `*.libraries.yml` and names no PHP class, the same shape a route or a permission
//! takes -- see the framework-side module docs for why, unlike routes and permissions, this one is
//! closed.

use std::path::Path;
use std::sync::Arc;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;

use super::{Query, check_view_with};
use crate::{IndexView, Place};

/// A render array attaching two libraries, the shape most of a real project's own code is.
const PROJECT: &str = r#"
//- /src/AppForm.php
<?php
namespace App\Form;

class AppForm
{
    public function build(): array
    {
        return [
            '#attached' => [
                'library' => ['app/@here@main', 'app/secondary'],
            ],
        ];
    }
}
"#;

/// The library catalog, closed, with two ids declared.
fn libraries(world: &mut World) {
    world.contracts_mut().bind_array_key("library", "library");
    for (key, line, detail) in [("app/main", 4_u32, "1.x"), ("app/secondary", 9, "1.x")] {
        world.contracts_mut().catalog_place(
            "library",
            key,
            Place {
                path: Arc::from(Path::new("/project/app.libraries.yml")),
                line,
                character: 0,
                detail: detail.into(),
            },
        );
    }
    world.contracts_mut().close_catalog("library");
}

#[test]
fn a_library_name_inside_the_list_is_the_cursor_target() {
    // The cursor sits on `main`, inside the second string of the list -- not on the whole list and
    // not on `#attached` or `library`, which is exactly what `keyed_element_at` reaching past the
    // array is for.
    check_view_with(
        PROJECT,
        &[Query::cursor("what it is", "here")],
        libraries,
        expect![[r#"
            what it is
            - catalog key app/main is declared at 5:1 (1.x)
        "#]],
    );
}

/// Without the binding the entries are a pair of strings, and the cursor is on nothing -- the same
/// answer an unbound `#type` gets.
#[test]
fn an_unbound_key_leaves_the_strings_alone() {
    check_view_with(
        PROJECT,
        &[Query::cursor("unbound", "here")],
        |_| {},
        expect![[r#"
            unbound
            - (nothing under the cursor)
        "#]],
    );
}

/// A name nothing declares is not a place to go, and -- unlike an unregistered `#type`, whose
/// catalog stays open -- it is a finding, because this catalog is closed.
#[test]
fn an_undeclared_name_answers_nothing_and_is_reported() {
    check_uses(
        r#"
//- /src/AppForm.php
<?php
namespace App\Form;

class AppForm
{
    public function build(): array
    {
        return [
            '#attached' => [
                'library' => ['app/@here@nowhere'],
            ],
        ];
    }
}
"#,
        libraries,
        expect![[r#"
            cursor
            - (nothing under the cursor)
            uses
            - library app/nowhere: unknown, catalog closed
        "#]],
    );
}

/// Every item of the list is its own use, not one use for the whole array -- the fact the sweep
/// would silently lose if the list-exploding entries collapsed back to the array's own range.
#[test]
fn every_item_of_the_list_is_its_own_use() {
    check_uses(
        PROJECT,
        libraries,
        expect![[r#"
            cursor
            - catalog key app/main is declared at 5:1 (1.x)
            uses
            - library app/main: known, catalog closed
            - library app/secondary: known, catalog closed
        "#]],
    );
}

/// The whole-file sweep and the cursor answer, from one fixture -- the bug worth catching is one of
/// them drifting from the other.
fn check_uses(fixture: &str, state: impl FnOnce(&mut World), expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    state(&mut world);
    let world = world;

    let cursor = spec.markers().position("here");
    let file = world.file_id(&cursor.path);
    let offset = cursor.offset;

    let answer = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        render(&view, offset)
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&answer);
}

fn render(view: &IndexView<'_>, offset: phrost_model::TextSize) -> String {
    let mut out = String::from("cursor\n");
    match view.cursor_target(offset) {
        Some(target) => out.push_str(&format!("- {}\n", target.label())),
        None => out.push_str("- (nothing under the cursor)\n"),
    }

    out.push_str("uses\n");
    for used in view.catalog_uses() {
        out.push_str(&format!(
            "- {} {}: {}, catalog {}\n",
            used.catalog,
            used.key,
            if used.known { "known" } else { "unknown" },
            if used.closed { "closed" } else { "open" }
        ));
    }

    out
}
