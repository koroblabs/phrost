//! A generic class a hint names bare, filled by the chain the body builds from a class.
//!
//! The repository shape: `run(): Paginator` whose body is `return User::query()->with(...)->
//! paginate(20);`, or the same through a local the body gave the start of it once. The hint says a
//! paginator and only the chain says of what. A chain from `$this` was read already; one built from
//! a class was not, so every repository's page was of nothing.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_chain_built_from_a_class_fills_the_generic_a_hint_left_bare() {
    check_view(
        r#"
//- /src/Orm.php
<?php
namespace App;

/**
 * @template TKey
 * @template TValue
 */
class Paginator {}

/** @template TModel */
class Builder
{
    /** @return $this */
    public function with(string $relation) { return $this; }

    /** @return Paginator<int, TModel> */
    public function paginate(int $count) { return new Paginator(); }
}

class User
{
    /** @return Builder<static> */
    public static function query() { return new Builder(); }
}

class Users
{
    public function direct(): Paginator
    {
        return User::query()->with('roles')->paginate(20);
    }

    public function throughALocal(bool $search): Paginator
    {
        $query = User::query()->with('roles');
        if ($search) {
            $query->with('avatar');
        }
        return $query->paginate(20);
    }
}

//- /src/run.php
<?php
namespace App;

function run(Users $users): void
{
    $direct = $users->direct();
    $d@direct@irect;
    $local = $users->throughALocal(true);
    $l@local@ocal;
}
"#,
        &[
            Query::variable_type("built in the return", "direct"),
            Query::variable_type("built in a local", "local"),
        ],
        expect![[r#"
            built in the return
            - Paginator<int, User>
            built in a local
            - Paginator<int, User>
        "#]],
    );
}
