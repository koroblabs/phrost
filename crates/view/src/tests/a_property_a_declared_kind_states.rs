//! `$node->get('field_address')->state`, where the field's kind is one the project declares itself.
//!
//! The list's `__get` answers every name, and the table of the kind says what `state` holds -- but
//! goto had nowhere to go and the access counted as one the magic excused. The class whose body
//! states the table is where `state` is written, so the access is owned by that class and lands on
//! the key. Neutral names, as in the other tests of a declared table.

use expect_test::expect;
use phrost_harness::World;

use super::{Query, check_view_ingested_with};

fn fields(world: &mut World) {
    world.add_declared_table(
        "Element",
        "shapeOf",
        &["Slot"],
        &[("text", phrost_model::BuiltinTy::String)],
        "field",
    );
    world
        .contracts_mut()
        .bind_table("Dep\\ListInterface", "get", "field", None);
    world
        .contracts_mut()
        .add_catalog_kind("field", "field_address", "address");
}

#[test]
fn a_read_through_the_list_lands_on_the_key_the_kind_states() {
    check_view_ingested_with(
        r#"
//- /src/Dep.php
<?php
namespace Dep;

interface ListInterface
{
    public function __get(string $name): mixed;
}

interface EntityInterface
{
    public function get(string $name): ListInterface;
}

//- /src/Address.php
<?php
namespace App;

#[Element('address')]
class AddressItem
{
    public static function shapeOf($definition): array
    {
        $slots['state'] = Slot::create('text');
        return $slots;
    }
}

//- /src/app.php
<?php
namespace App;

use Dep\EntityInterface;

function run(EntityInterface $node): void
{
    $state = $node->get('field_address')->st@state@ate;
    $other = $node->get('field_address')->miss@missing@ing;
    $s@typed@tate;
}
"#,
        &[
            Query::goto("the key the kind's table writes", "state"),
            Query::goto("a name the table does not hold", "missing"),
            Query::variable_type("what the read holds", "typed"),
        ],
        fields,
        expect![[r#"
            the key the kind's table writes
            - property App\AddressItem::state @ src/Address.php 9:17-9:22
            a name the table does not hold
            - (not found)
            what the read holds
            - string
        "#]],
    );
}
