//! A `@mixin` a framework's class behaves as though it wrote.
//!
//! Eloquent's `Model` forwards every call it does not declare to a fresh `Builder<static>`, and
//! nothing on the class says so -- the forwarding is in `__call`. So `User::where(...)` is a call on
//! the builder: its `$this` is the builder, its templates are bound by the model it was made on, and
//! a method the model *does* declare is still the model's.

use expect_test::expect;

use super::{Query, check_view_ingested_with};

const FIXTURE: &str = r#"
//- /vendor/orm/Builder.php
<?php
namespace Orm;

/** @template TModel of Model */
class Builder
{
    /** What the builder holds, which a model's `__get` never reads. */
    public array $orders = [];

    /** @return $this */
    public function where(string $column, mixed $value) { return $this; }

    /** @return TModel|null */
    public function first() {}

    /**
     * @param  mixed  $id
     * @return ($id is (Arrayable<array-key, mixed>|array<mixed>) ? Collection<int, TModel> : TModel|null)
     */
    public function find($id, $columns = ['*']) {}

    public function save(int $flags): bool { return true; }
}

//- /vendor/orm/Collection.php
<?php
namespace Orm;

/** @template TKey @template TValue */
class Collection {}

interface Arrayable {}

//- /vendor/orm/Model.php
<?php
namespace Orm;

abstract class Model
{
    public function save(): bool { return true; }

    public function __call($method, $parameters) {}

    public static function __callStatic($method, $parameters) {}
}
"#;

fn model_forwards_to_its_builder(world: &mut phrost_harness::World) {
    world.add_implied_mixin("Orm\\Model", "Orm\\Builder", true);
}

#[test]
fn a_forwarded_call_answers_on_the_builder_bound_by_the_model() {
    check_view_ingested_with(
        &[
            FIXTURE,
            r#"
//- /src/User.php
<?php
namespace App;

class User extends \Orm\Model
{
    public function run(): void
    {
        $query = User::where('a', 1);
        $que@q@ry;
        $first = User::where('a', 1)->first();
        $fir@f@st;
        $found = User::find(1);
        $fou@d@nd;
        $many = User::find([1, 2]);
        $ma@m@ny;
    }

    /** @param \Orm\Builder<User> $b */
    public function probe($b): void
    {
        $x = $b->find(1);
        $@x@x;
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("a fluent call hands the builder back", "q"),
            Query::variable_type("a template the builder was made with", "f"),
            Query::variable_type("a conditional return settled by the call", "d"),
            Query::variable_type("and its other branch", "m"),
            Query::variable_type("the same contract on a builder written with its model", "x"),
        ],
        model_forwards_to_its_builder,
        expect![[r#"
            a fluent call hands the builder back
            - Builder<User>
            a template the builder was made with
            - ?User
            a conditional return settled by the call
            - ?User
            and its other branch
            - Collection<int, User>
            the same contract on a builder written with its model
            - ?User
        "#]],
    );
}

/// The model's own method is the one that runs, and PHP checks nothing between it and the
/// builder's: a mixin declares nothing above the class, so there is no override to fit.
///
/// And only the builder's methods are in the table: what the framework implies is a forwarded call,
/// and the model's `__get` reads its own attributes and relations -- so the builder's `$orders` is
/// not a model's, whose `orders()` relation is what `$user->orders` means.
#[test]
fn a_member_the_model_declares_is_its_own_and_overrides_nothing() {
    check_view_ingested_with(
        &[
            FIXTURE,
            r#"
//- /src/User.php
<?php
namespace App;

class User extends \Orm\Model {}
"#,
        ]
        .concat(),
        &[Query::members("the table", "App\\User")],
        model_forwards_to_its_builder,
        expect![[r#"
            the table
            - method __call($method, $parameters)  public  [inherited Orm\Model]
            - method __callStatic($method, $parameters)  public static  [inherited Orm\Model]
            - method find(mixed $id, $columns = ['*']): Collection<int, TModel>|TModel|null  public  [inherited Orm\Builder]
            - method first(): ?TModel  public  [inherited Orm\Builder]
            - method save(): bool  public  [inherited Orm\Model]
            - method where(string $column, mixed $value): static  public  [inherited Orm\Builder]
        "#]],
    );
}
