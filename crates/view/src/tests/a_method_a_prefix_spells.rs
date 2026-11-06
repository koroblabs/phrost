//! A method `__call` answers for every name that begins with a prefix, handing back the receiver.
//!
//! Laravel's dynamic wheres: `User::whereEmail($email)->first()`. The query builder reads the column
//! off the name; the Eloquent builder reaches it through its `@mixin`, and a model through the
//! builder it creates -- so the same call answers three different receivers, and each is the value
//! the chain goes on with.

use expect_test::expect;

use super::{Query, check_view_ingested_with};

const FIXTURE: &str = r#"
//- /vendor/orm/Query.php
<?php
namespace Orm;

class Query
{
    /** @return $this */
    public function where(string $column, mixed $value) { return $this; }

    public function __call($method, $parameters) {}
}

//- /vendor/orm/Builder.php
<?php
namespace Orm;

/**
 * @template TModel of Model
 * @mixin Query
 */
class Builder
{
    /** @return TModel|null */
    public function first() {}

    public function __call($method, $parameters) {}
}

//- /vendor/orm/Model.php
<?php
namespace Orm;

abstract class Model
{
    public function __call($method, $parameters) {}

    public static function __callStatic($method, $parameters) {}
}
"#;

fn wheres_are_dynamic(world: &mut phrost_harness::World) {
    world.add_implied_mixin("Orm\\Model", "Orm\\Builder", true);
    world
        .contracts_mut()
        .bind_fluent_prefix("Orm\\Query", "where");
}

#[test]
fn a_prefixed_name_answers_with_the_receiver_it_was_called_on() {
    check_view_ingested_with(
        &[
            FIXTURE,
            r#"
//- /src/User.php
<?php
namespace App;

class User extends \Orm\Model
{
    /** @param \Orm\Builder<User> $builder */
    public function run(\Orm\Query $query, $builder): void
    {
        $a = $query->whereEmail('x');
        $@a@a;
        $b = $builder->whereEmail('x');
        $@b@b;
        $c = User::whereEmail('x');
        $@c@c;
        $d = User::whereEmail('x')->whereName('y')->first();
        $@d@d;
        $e = $query->wherever();
        $@e@e;
        $f = $query->limitEmail('x');
        $@f@f;
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("the class whose `__call` reads the name", "a"),
            Query::variable_type("a class mixing it in is still the receiver", "b"),
            Query::variable_type("a model forwards to the builder it creates", "c"),
            Query::variable_type("and the chain goes on", "d"),
            Query::variable_type("the prefix not followed by a word", "e"),
            Query::variable_type("another prefix", "f"),
        ],
        wheres_are_dynamic,
        expect![[r#"
            the class whose `__call` reads the name
            - Query
            a class mixing it in is still the receiver
            - Builder<User>
            a model forwards to the builder it creates
            - Builder<User>
            and the chain goes on
            - ?User
            the prefix not followed by a word
            - (no type)
            another prefix
            - (no type)
        "#]],
    );
}
