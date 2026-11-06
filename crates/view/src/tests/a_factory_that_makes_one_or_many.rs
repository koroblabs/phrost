//! A model factory, whose `create()` is one model unless the chain counted.
//!
//! Declared `Collection<int, TModel>|TModel`, which is honest about the method and says nothing
//! about the call: `User::factory()->create()` is a `User`, `->count(3)->create()` and
//! `User::factory(3)->create()` a collection. Read as the union, every model a test made and handed
//! to a parameter typed by its class was a collection that might arrive there. A count handed over
//! as a value -- `factory($n)` -- may be either, and keeps the union; so does a factory the chain
//! did not start, whose count was set where this cannot see.

use expect_test::expect;

use phrost_index::ChainedOperandRule;

use super::{Query, check_view_with};

#[test]
fn the_chain_says_whether_a_factory_counted() {
    check_view_with(
        r#"
//- /vendor/Orm.php
<?php
namespace Orm;

/**
 * @template TKey
 * @template TValue
 */
class Collection {}

/** @template TModel */
abstract class Factory
{
    /** @return static */
    public function count(?int $count): static {}

    /** @return Collection<int, TModel>|TModel */
    public function create() {}
}

//- /src/User.php
<?php
namespace App;

use Orm\Factory;

class User
{
    public static function factory(?int $count = null): UserFactory {}
}

/** @extends Factory<User> */
class UserFactory extends Factory {}

function run(int $n): void
{
    $o@one@ne = User::factory()->create();
    $c@counted@ounted = User::factory()->count(3)->create();
    $a@argument@rgument = User::factory(2)->create();
    $u@unknown@nknown = User::factory($n)->create();
    $factory = User::factory()->count(3);
    $v@variable@ariable = $factory->create();
}
"#,
        &[
            Query::variable_type("nothing counted", "one"),
            Query::variable_type("count() in the chain", "counted"),
            Query::variable_type("a count handed to factory()", "argument"),
            Query::variable_type("a count nothing can read", "unknown"),
            Query::variable_type("a factory started somewhere else", "variable"),
        ],
        |world| {
            world.contracts_mut().add_chained_operand(
                "create",
                ChainedOperandRule {
                    class: "Orm\\Factory".into(),
                    after: vec!["count".into()],
                    counted_by: vec!["factory".into()],
                    many: "Orm\\Collection".into(),
                },
            );
        },
        expect![[r#"
            nothing counted
            - User
            count() in the chain
            - Collection<int, User>
            a count handed to factory()
            - Collection<int, User>
            a count nothing can read
            - Collection<int, User>|User
            a factory started somewhere else
            - Collection<int, User>|User
        "#]],
    );
}
