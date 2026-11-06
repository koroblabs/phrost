//! The other half of a framework's array: the keys beside the id, not the id.
//!
//! `['#type' => 'textfield', '#|']`. A catalog id is offered in a *value*, and these are offered in
//! the *key* beside it -- names that appear in no declaration anywhere, only in the prose of the
//! class the id chose. So the dispatch is what these hold: a string inside a render array gets the
//! documented keys, a string in the same array's value position still gets the ids, and a string
//! anywhere else still gets the silence every other literal gets.

use expect_test::expect;

use crate::tests::{Query, check, check_with};

/// Two element classes and the base above them, documented the way the real ones are: a bullet list
/// with no heading over it, and the properties everybody writes on the base rather than the leaf.
const FORM: &str = r#"
//- /src/ElementBase.php
<?php
namespace App\Element;

/**
 * Base class for form elements.
 *
 * Here is a list of properties used during rendering:
 * - #title: (string) The label for the element.
 * - #required: (bool) Whether the element must be filled in.
 */
class ElementBase {}

//- /src/Textfield.php
<?php
namespace App\Element;

/**
 * Provides a one-line text field.
 *
 * Properties:
 * - #maxlength: (int) Maximum number of characters allowed.
 */
class Textfield extends ElementBase {}

//- /src/AppForm.php
<?php
namespace App\Form;

class AppForm
{
    public function build(): array
    {
        return [
            '#type' => 'textfield',
            '#@here@',
        ];
    }
}
"#;

fn elements(world: &mut phrost_harness::World) {
    world.contracts_mut().bind_array_key("#type", "element");
    world
        .contracts_mut()
        .document_keys_under("#type", "element", "#");
    world
        .contracts_mut()
        .catalog_entry("element", "textfield", Some("App\\Element\\Textfield"));
}

#[test]
fn the_keys_the_chosen_element_documents_are_offered_with_what_they_hold() {
    check_with(
        FORM,
        &[Query::completion("properties", "here")],
        elements,
        expect![[r#"
            properties
            - #maxlength [property]  int
            - #title [property]  string
            - #required [property]  bool
            - accepting the first leaves `'#maxlength',`
        "#]],
    );
}

/// Without the convention the array is a pair of strings, which is what it was before any of this.
#[test]
fn a_project_with_no_such_convention_offers_nothing_in_the_key() {
    check(
        FORM,
        &[Query::completion("properties", "here")],
        expect![[r#"
            properties
            - (nothing offered)
        "#]],
    );
}
