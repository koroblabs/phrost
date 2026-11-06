//! What a table states, offered where goto already reaches it.
//!
//! `$entity->get('field_address')->state` goes to the key the field kind's table writes, and hover
//! describes it -- but `->st` offered the list interface's methods and nothing the table holds,
//! because completion listed the member table and the table is not in it. Found by asking the
//! three other questions at every word goto answers: `->entity`, `->state`, `->json`, `->number`
//! and `$this->value` on one real project, every one of them a read the index types. Neutral names,
//! as in the view's tests of a declared table.

use expect_test::expect;
use phrost_harness::World;

use crate::tests::{Query, check_with};

fn fields(world: &mut World) {
    world.add_declared_table(
        "Element",
        "shapeOf",
        &["Slot"],
        &[
            ("text", phrost_model::BuiltinTy::String),
            ("flag", phrost_model::BuiltinTy::Bool),
        ],
        "field",
    );
    world
        .contracts_mut()
        .bind_table("Dep\\ListInterface", "get", "field", None);
    world
        .contracts_mut()
        .add_catalog_kind("field", "field_address", "address");
}

const DECLARATIONS: &str = r#"
//- /src/Dep.php
<?php
namespace Dep;

interface ListInterface
{
    public function __get(string $name): mixed;
    public function isEmpty(): bool;
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
    public function __get(string $name): mixed { return null; }

    public static function shapeOf($definition): array
    {
        $slots['state'] = Slot::create('text');
        $slots['verified'] = Slot::create('flag');
        return $slots;
    }
}
"#;

#[test]
fn a_list_a_keyed_call_produced_offers_what_its_kind_holds() {
    check_with(
        &format!(
            "{DECLARATIONS}{}",
            r#"
//- /src/app.php
<?php
namespace App;

use Dep\EntityInterface;

function run(EntityInterface $node): void
{
    $node->get('field_address')->@here@
}
"#
        ),
        &[Query::completion("through the list", "here")],
        fields,
        expect![[r#"
            through the list
            - __get [method]  : mixed  shown last  writes `(|);`
            - isEmpty [method]  : bool  writes `();`
            - state [property]  string
            - verified [property]  bool
            - accepting the first leaves `$node->get('field_address')->isEmpty`
        "#]],
    );
}

#[test]
fn a_local_the_list_was_put_in_offers_the_same() {
    check_with(
        &format!(
            "{DECLARATIONS}{}",
            r#"
//- /src/app.php
<?php
namespace App;

use Dep\EntityInterface;

function run(EntityInterface $node): void
{
    $field = $node->get('field_address');
    $field->ver@here@
}
"#
        ),
        &[Query::completion("through a local", "here")],
        fields,
        expect![[r#"
            through a local
            - verified [property]  bool
            - ... and the client must ask again
            - accepting the first leaves `$field->verified`
        "#]],
    );
}

#[test]
fn the_class_whose_body_states_the_table_offers_it_on_itself() {
    check_with(
        &format!(
            "{DECLARATIONS}{}",
            r#"
//- /src/Reader.php
<?php
namespace App;

class Reader
{
    public function read(AddressItem $item): void
    {
        $item->st@here@
    }
}
"#
        ),
        &[Query::completion("on an item", "here")],
        fields,
        expect![[r#"
            on an item
            - state [property]  string
            - ... and the client must ask again
            - accepting the first leaves `$item->state`
        "#]],
    );
}

/// A key whose kind nobody declared, and a call no binding speaks for, offer the list's own
/// members and nothing more -- the table is the key's, not the list's.
#[test]
fn a_key_of_no_known_kind_offers_only_the_list() {
    check_with(
        &format!(
            "{DECLARATIONS}{}",
            r#"
//- /src/app.php
<?php
namespace App;

use Dep\EntityInterface;

function run(EntityInterface $node): void
{
    $node->get('field_unknown')->@here@
}
"#
        ),
        &[Query::completion("an unknown key", "here")],
        fields,
        expect![[r#"
            an unknown key
            - __get [method]  : mixed  shown last  writes `(|);`
            - isEmpty [method]  : bool  writes `();`
            - accepting the first leaves `$node->get('field_unknown')->isEmpty`
        "#]],
    );
}

/// The other source goto reads: a binding, which says one class for every name it speaks for.
fn bindings(world: &mut World) {
    let contracts = world.contracts_mut();
    contracts.bind_property(
        "Dep\\ListInterface",
        phrost_harness::PropertyNames::Exactly("entity".into()),
        "Dep\\EntityInterface",
        true,
    );
    contracts.bind_property(
        "Dep\\EntityInterface",
        phrost_harness::PropertyNames::KeyOf("field".into()),
        "Dep\\ListInterface",
        false,
    );
    for key in ["field_address", "field_tags"] {
        contracts.catalog_entry("field", key, None);
    }
}

#[test]
fn a_name_a_binding_speaks_for_is_offered_with_what_it_holds() {
    check_with(
        &format!(
            "{DECLARATIONS}{}",
            r#"
//- /src/app.php
<?php
namespace App;

use Dep\ListInterface;

function run(ListInterface $list): void
{
    $list->ent@here@
}
"#
        ),
        &[Query::completion("a bound name", "here")],
        bindings,
        expect![[r#"
            a bound name
            - entity [property]  ?EntityInterface
            - ... and the client must ask again
            - accepting the first leaves `$list->entity`
        "#]],
    );
}

/// Every key of the catalog, after the declared members: on an entity those are every field the
/// project has, and a person reaching for `get()` should not have to scroll past them.
#[test]
fn the_keys_a_binding_answers_for_come_after_the_declared_members() {
    check_with(
        &format!(
            "{DECLARATIONS}{}",
            r#"
//- /src/app.php
<?php
namespace App;

use Dep\EntityInterface;

function run(EntityInterface $node): void
{
    $node->@here@
}
"#
        ),
        &[Query::completion("an entity", "here")],
        bindings,
        expect![[r#"
            an entity
            - get [method]  : ListInterface  writes `(|);`
            - field_address [property]  ListInterface  shown last
            - field_tags [property]  ListInterface  shown last
            - accepting the first leaves `$node->get`
        "#]],
    );
}
