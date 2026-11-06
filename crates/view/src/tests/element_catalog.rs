//! A key written as an array entry rather than passed to a call.
//!
//! `['#type' => 'textfield']`. Everything else in this crate that resolves a string hangs off a
//! call -- a receiver, a method name, a first argument -- and a render array has none of those. The
//! key beside the value is the whole contract, which is what
//! [`bind_array_key`](phrost_index::ReturnContracts::bind_array_key) exists to state.
//!
//! One binding buys goto, hover and completion at once, because all three go through the same
//! search for the literal and the catalogs it indexes. So all three are checked here, from one
//! fixture: a change that reaches the cursor and not the completion is exactly the split this file
//! is for.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_model::TextSize;
use phrost_testkit::FixtureSpec;

use super::{Query, check_view_with};
use crate::IndexView;

/// A form-building method, which is what most of a Drupal project's own code looks like.
const PROJECT: &str = r#"
//- /src/Textfield.php
<?php
namespace App\Element;

class Textfield {}

//- /src/AppForm.php
<?php
namespace App\Form;

class AppForm
{
    public function build(): array
    {
        return [
            'name' => [
                '#type' => 'text@here@field',
                '#title' => 'Name',
            ],
        ];
    }
}
"#;

/// The element catalog, with one id registered.
fn elements(world: &mut World) {
    world.contracts_mut().bind_array_key("#type", "element");
    world
        .contracts_mut()
        .catalog_entry("element", "textfield", Some("App\\Element\\Textfield"));
    world
        .contracts_mut()
        .catalog_entry("element", "textarea", Some("App\\Element\\Textarea"));
}

#[test]
fn an_element_id_is_the_class_it_names() {
    check_view_with(
        PROJECT,
        &[
            Query::cursor("what it is", "here"),
            Query::goto("where it goes", "here"),
        ],
        elements,
        expect![[r#"
            what it is
            - catalog key textfield is App\Element\Textfield
            where it goes
            - class App\Element\Textfield @ src/Textfield.php 4:7-4:16
        "#]],
    );
}

/// Without the binding the entry is a pair of strings, and the cursor is on nothing.
#[test]
fn an_unbound_key_leaves_the_string_alone() {
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

/// A key the catalog does not hold answers nothing -- and is not reported, because the catalog was
/// never closed. See the module documentation on the framework side for why it cannot be.
#[test]
fn an_unregistered_id_is_nothing_and_is_not_a_finding() {
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
            '#type' => 'nothing_registers@here@_this',
            '#title' => 'Name',
        ];
    }
}
"#,
        elements,
        // The site is still a site -- a person editing the id is offered the list, and it is the
        // prefix that matches nothing rather than the entry.
        expect![[r#"
            uses
            - element nothing_registers_this: unknown, catalog open
            keys offered, replacing `nothing_registers_this`
        "#]],
    );
}

/// A key written under a *different* array key is a plain string. `#title` holds a label, and
/// offering element ids there would be worse than offering nothing.
#[test]
fn only_the_bound_key_is_read_as_one() {
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
            '#title' => 'text@here@field',
        ];
    }
}
"#,
        elements,
        expect![[r#"
            uses
            keys offered
            - (no keys offered)
        "#]],
    );
}

/// What the whole-file sweep sees, and what a person typing in the value is offered.
///
/// One fixture answering both, because they read the same thing two ways and the bug worth catching
/// is one of them drifting from the other.
#[test]
fn the_sweep_and_the_completion_read_the_same_entry() {
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
            '#type' => 'text@here@',
            '#title' => 'Name',
        ];
    }
}
"#,
        elements,
        expect![[r#"
            uses
            - element text: unknown, catalog open
            keys offered, replacing `text`
            - textarea (App\Element\Textarea)
            - textfield (App\Element\Textfield)
        "#]],
    );
}

/// Every catalog use in the file, and the keys offered at `@here@` when the fixture marks one.
///
/// Whole results, both of them: an extra offer has to be as visible as a missing one, and a report
/// that grew a finding has to show it.
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

fn render(view: &IndexView<'_>, offset: TextSize) -> String {
    let mut out = String::from("uses\n");
    for used in view.catalog_uses() {
        let _ = writeln!(
            out,
            "- {} {}: {}, catalog {}",
            used.catalog,
            used.key,
            if used.known { "known" } else { "unknown" },
            if used.closed { "closed" } else { "open" }
        );
    }

    match view.catalog_keys_at(offset, 32) {
        Some((written, items)) => {
            let _ = writeln!(
                out,
                "keys offered, replacing `{}`",
                view.current()
                    .and_then(|open| open
                        .text()
                        .get(usize::from(written.start())..usize::from(written.end())))
                    .unwrap_or("")
            );
            for item in items {
                let _ = writeln!(out, "- {} ({})", item.key, item.detail);
            }
        }
        None => out.push_str("keys offered\n- (no keys offered)\n"),
    }

    out
}
