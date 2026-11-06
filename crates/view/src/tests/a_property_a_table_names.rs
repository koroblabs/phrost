//! A property whose type is a table lookup, keyed by a string written two links back.
//!
//! `$node->get('field_when')->value` is a `string` and `$node->get('field_count')->value` is an
//! `int`. The interface declares neither, and the same read on the same class answers differently
//! for every key -- which is why the rule is a lookback rather than a property with a type.
//!
//! Every case in one fixture and one table, because what decides this feature is the *pairs*: two
//! keys of one class, a key nothing registers, a table that is partial, and a receiver no earlier
//! call named. Each of those is an absence in a test of its own and reads as coverage.

use expect_test::expect;

use super::{Query, check_view_with};
use phrost_harness::World;

/// A framework that says a field list is tabled, with three tables in it.
fn fields(world: &mut World) {
    world
        .contracts_mut()
        .bind_table("Dep\\ListInterface", "get", "field", Some("getValue"));
    // The kinds first: a table is keyed by kind, and a key with no kind finds none.
    world
        .contracts_mut()
        .add_catalog_kind("field", "field_when", "datetime");
    world
        .contracts_mut()
        .add_catalog_kind("field", "field_count", "integer");
    world
        .contracts_mut()
        .add_catalog_kind("field", "field_photo", "image");
    world.contracts_mut().add_property_table(
        "field",
        "datetime",
        vec![
            ("value".into(), Some(phrost_model::BuiltinTy::String)),
            // A property the declaration names and says nothing PHP has a word for. The key is
            // there; what is in it is not claimed.
            ("date".into(), None),
        ],
        true,
    );
    world.contracts_mut().add_property_table(
        "field",
        "integer",
        vec![("value".into(), Some(phrost_model::BuiltinTy::Int))],
        true,
    );
    // Partial: it answers for a name it holds and may never become a shape.
    world.contracts_mut().add_property_table(
        "field",
        "image",
        vec![("alt".into(), Some(phrost_model::BuiltinTy::String))],
        false,
    );
    world.intern_property_shapes();
}

const FIXTURE: &str = r#"
//- /src/Dep.php
<?php
namespace Dep;

interface ListInterface
{
    public function getValue(): array;
}

interface EntityInterface
{
    public function get(string $name): ListInterface;
}
"#;

#[test]
fn two_keys_of_one_class_answer_differently() {
    check_view_with(
        &[
            FIXTURE,
            r#"
//- /src/app.php
<?php
namespace App;

use Dep\EntityInterface;

class Reader
{
    public function read(EntityInterface $node): void
    {
        $when = $node->get('field_when')->value;
        $w@when@hen;
        $count = $node->get('field_count')->value;
        $c@count@ount;
        $unsaid = $node->get('field_when')->date;
        $u@unsaid@nsaid;
        $absent = $node->get('field_when')->missing;
        $a@absent@bsent;
        $unregistered = $node->get('field_nothing')->value;
        $un@unregistered@registered;
        $alt = $node->get('field_photo')->alt;
        $al@alt@t;
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("a string field", "when"),
            Query::variable_type("an int field", "count"),
            Query::variable_type("a property the table names and cannot type", "unsaid"),
            Query::variable_type("a name the table does not hold", "absent"),
            Query::variable_type("a key nothing registers", "unregistered"),
            Query::variable_type("a partial table still answers for a name it holds", "alt"),
        ],
        fields,
        expect![[r#"
            a string field
            - string
            an int field
            - int
            a property the table names and cannot type
            - (no type)
            a name the table does not hold
            - (no type)
            a key nothing registers
            - (no type)
            a partial table still answers for a name it holds
            - string
        "#]],
    );
}

#[test]
fn the_rows_method_answers_with_the_whole_shape() {
    check_view_with(
        &[
            FIXTURE,
            r#"
//- /src/app.php
<?php
namespace App;

use Dep\EntityInterface;

class Reader
{
    public function read(EntityInterface $node): void
    {
        $rows = $node->get('field_when')->getValue();
        $r@rows@ows;
        // A partial table may never become a shape: it claims the names it lists are the names
        // there are, and this one says it does not know them all.
        $partial = $node->get('field_photo')->getValue();
        $p@partial@artial;
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("one entry per delta, each the whole shape", "rows"),
            Query::variable_type("a partial table is not a shape", "partial"),
        ],
        fields,
        expect![[r#"
            one entry per delta, each the whole shape
            - array<int, array{value: string, date: mixed}>
            a partial table is not a shape
            - array
        "#]],
    );
}

/// A receiver no earlier call named has no key to look up, and the declaration stands.
#[test]
fn a_receiver_no_call_produced_keeps_what_it_declares() {
    check_view_with(
        &[
            FIXTURE,
            r#"
//- /src/app.php
<?php
namespace App;

use Dep\ListInterface;

class Reader
{
    public function read(ListInterface $list): void
    {
        $loose = $list->value;
        $l@loose@oose;
        $rows = $list->getValue();
        $r@rows@ows;
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("a property with no key behind it", "loose"),
            Query::variable_type("and the rows method with none either", "rows"),
        ],
        fields,
        expect![[r#"
            a property with no key behind it
            - (no type)
            and the rows method with none either
            - array
        "#]],
    );
}
