//! The keys an array's own type key says it accepts.
//!
//! `['#type' => 'textfield', '#ma|']`. Nothing declares `#maxlength` anywhere -- it is not a
//! property, not a constant, not an argument -- and the only statement that it exists is a bullet in
//! the docblock of the class `textfield` names. So this is the one completion in the crate whose
//! answer comes out of prose, and the tests are as much about where it stays quiet: an array with no
//! type key, a type key naming nothing, and a project whose framework contributed no convention all
//! have to answer nothing rather than offer a form element's properties.
//!
//! The inherited half is the load-bearing one. A leaf element documents its own three properties and
//! inherits the twenty everybody actually writes, so a walk that stopped at the class named would
//! offer the rare ones and lose `#title`.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_model::TextSize;
use phrost_testkit::FixtureSpec;

use crate::IndexView;

/// Two element classes and the base above them, documented the way the real ones are: a bullet list
/// with no heading over it, a parenthesised kind on some entries and not others, and a description
/// that runs over two lines.
const ELEMENTS: &str = r#"
//- /src/ElementBase.php
<?php
namespace App\Element;

/**
 * Base class for form elements.
 *
 * Here is a list of properties used during rendering, besides those documented
 * on the render element:
 * - #title: (string) The label for the element. Shown above the input.
 * - #required: (bool) Whether the element must be filled in.
 *
 * @see \App\Element\Textfield
 */
class ElementBase {}

//- /src/Textfield.php
<?php
namespace App\Element;

/**
 * Provides a one-line text field.
 *
 * Properties:
 * - #maxlength: Maximum number of characters allowed.
 * - #size: (int) The width of the input in characters.
 */
class Textfield extends ElementBase {}

//- /src/Textarea.php
<?php
namespace App\Element;

/**
 * Provides a multi-line text field.
 *
 * Properties:
 * - #rows: (int) How many rows the box is tall.
 */
class Textarea extends ElementBase {}
"#;

/// The catalog and the convention, as the Drupal provider contributes them.
fn elements(world: &mut World) {
    world.contracts_mut().bind_array_key("#type", "element");
    world
        .contracts_mut()
        .document_keys_under("#type", "element", "#");
    world
        .contracts_mut()
        .catalog_entry("element", "textfield", Some("App\\Element\\Textfield"));
    world
        .contracts_mut()
        .catalog_entry("element", "textarea", Some("App\\Element\\Textarea"));
}

#[test]
fn the_class_the_type_key_names_and_every_class_above_it_are_read() {
    check(
        &format!(
            r#"{ELEMENTS}
//- /src/AppForm.php
<?php
namespace App\Form;

class AppForm
{{
    public function build(): array
    {{
        return [
            '#type' => 'textfield',
            '@here@',
        ];
    }}
}}
"#
        ),
        elements,
        expect![[r#"
            offered, replacing ``
            - #maxlength [] Maximum number of characters allowed.
            - #size [int] The width of the input in characters.
            - #title [string] The label for the element.
            - #required [bool] Whether the element must be filled in.
        "#]],
    );
}

/// The keys already written are the ones that cannot be the answer, and a duplicate array key is the
/// one mistake this list could actively cause.
#[test]
fn a_key_the_array_already_writes_is_not_offered_back() {
    check(
        &format!(
            r#"{ELEMENTS}
//- /src/AppForm.php
<?php
namespace App\Form;

class AppForm
{{
    public function build(): array
    {{
        return [
            '#type' => 'textarea',
            '#title' => 'Notes',
            '@here@',
        ];
    }}
}}
"#
        ),
        elements,
        expect![[r#"
            offered, replacing ``
            - #rows [int] How many rows the box is tall.
            - #required [bool] Whether the element must be filled in.
        "#]],
    );
}

/// The innermost array, which is the one being written. The outer form's own type says nothing about
/// a child element's properties.
#[test]
fn a_nested_array_is_read_against_its_own_type_and_not_its_parents() {
    check(
        &format!(
            r#"{ELEMENTS}
//- /src/AppForm.php
<?php
namespace App\Form;

class AppForm
{{
    public function build(): array
    {{
        return [
            '#type' => 'textfield',
            'notes' => [
                '#type' => 'textarea',
                '@here@',
            ],
        ];
    }}
}}
"#
        ),
        elements,
        expect![[r#"
            offered, replacing ``
            - #rows [int] How many rows the box is tall.
            - #title [string] The label for the element.
            - #required [bool] Whether the element must be filled in.
        "#]],
    );
}

/// A `#type` whose value the catalog does not hold. Its class is unknown, so what the array accepts
/// is unknown -- and the catalog is open, so this is an ordinary thing rather than a mistake.
#[test]
fn an_array_whose_type_names_nothing_is_answered_with_nothing() {
    check(
        &format!(
            r#"{ELEMENTS}
//- /src/AppForm.php
<?php
namespace App\Form;

class AppForm
{{
    public function build(): array
    {{
        return [
            '#type' => 'Object',
            '@here@',
        ];
    }}
}}
"#
        ),
        elements,
        expect![[r#"
            nothing offered
        "#]],
    );
}

/// Without the convention there is nothing to read, however Drupal-shaped the array looks. The
/// binding alone says an id is an id; it does not say the class it names documents its neighbours.
#[test]
fn a_project_whose_framework_contributed_no_convention_is_answered_with_nothing() {
    check(
        &format!(
            r#"{ELEMENTS}
//- /src/AppForm.php
<?php
namespace App\Form;

class AppForm
{{
    public function build(): array
    {{
        return [
            '#type' => 'textfield',
            '@here@',
        ];
    }}
}}
"#
        ),
        |world| {
            world.contracts_mut().bind_array_key("#type", "element");
            world.contracts_mut().catalog_entry(
                "element",
                "textfield",
                Some("App\\Element\\Textfield"),
            );
        },
        expect![[r#"
            nothing offered
        "#]],
    );
}

/// What is offered at `@here@`, whole: an extra offer has to be as visible as a missing one.
fn check(fixture: &str, state: impl FnOnce(&mut World), expect: Expect) {
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
    let Some(found) = view.documented_keys_at(offset) else {
        return String::from("nothing offered\n");
    };

    let mut out = format!("offered, replacing `{}`\n", found.prefix);
    for key in found
        .keys
        .iter()
        .filter(|key| !found.written.iter().any(|held| held.as_str() == &*key.name))
    {
        let _ = writeln!(
            out,
            "- {} [{}] {}",
            key.name,
            key.kind.as_deref().unwrap_or(""),
            key.summary
        );
    }
    out
}
