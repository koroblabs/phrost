//! What a callback is handed, where the callee's signature names a template of the trait that wrote
//! it.
//!
//! Eloquent's `chunkById(callable(Collection<int, TValue>, int))` is written on `BuildsQueries`,
//! which the builder uses as `BuildsQueries<TModel>` -- so on a `Builder<User>` the callback is handed
//! a collection of users, by the same ladder `first()` returning `TValue|null` already went through.
//! Read without it the callback was handed a collection of a class called `TValue`, and every
//! element read off it was an unknown class.

use expect_test::expect;

use super::{Query, check_view_with};

const FIXTURE: &str = r#"
//- /vendor/orm/Collection.php
<?php
namespace Orm;

/** @template TKey @template TValue */
class Collection implements \IteratorAggregate {}

//- /vendor/orm/BuildsQueries.php
<?php
namespace Orm;

/** @template TValue */
trait BuildsQueries
{
    /** @param callable(Collection<int, TValue>, int): mixed $callback */
    public function chunk(int $count, callable $callback): bool { return true; }
}

//- /vendor/orm/Builder.php
<?php
namespace Orm;

/** @template TModel */
class Builder
{
    /** @use BuildsQueries<TModel> */
    use BuildsQueries;
}

//- /vendor/orm/Query.php
<?php
namespace Orm;

class Row {}

class Query
{
    /** @use BuildsQueries<Row> */
    use BuildsQueries;
}

//- /vendor/orm/Model.php
<?php
namespace Orm;

abstract class Model
{
    public static function __callStatic($method, $parameters) {}
}

//- /src/User.php
<?php
namespace App;

class User extends \Orm\Model
{
    /** @param \Orm\Builder<User> $users */
    public function run(\Orm\Builder $users, \Orm\Query $rows, \Orm\Builder $bare): void
    {
        $users->chunk(10, function ($p@page@age) {
            foreach ($page as $one) {
                $o@one@ne;
            }
        });
        $rows->chunk(10, function ($r@rows@ows) {});
        User::chunk(10, function ($s@forwarded@tatic) {});
        $bare->chunk(10, function ($b@bare@are) {});
    }
}
"#;

#[test]
fn a_callback_is_handed_what_the_receiver_binds_the_template_to() {
    check_view_with(
        FIXTURE,
        &[
            Query::variable_type("on a builder of users", "page"),
            Query::variable_type("and each element of it", "one"),
            Query::variable_type("a class binding the trait to a class of its own", "rows"),
            Query::variable_type("a static call a model forwards to its builder", "forwarded"),
            Query::variable_type("a receiver binding nothing: the template's bound", "bare"),
        ],
        |world| world.add_implied_mixin("Orm\\Model", "Orm\\Builder", true),
        expect![[r#"
            on a builder of users
            - Collection<int, User>
            and each element of it
            - User
            a class binding the trait to a class of its own
            - Collection<int, Row>
            a static call a model forwards to its builder
            - Collection<int, User>
            a receiver binding nothing: the template's bound
            - Collection<int, mixed>
        "#]],
    );
}
