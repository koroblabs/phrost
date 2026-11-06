//! A member two mixins forward answers from the nearer one.
//!
//! Eloquent's shape again: `Relation` writes `@mixin Builder<TRelated>` and implements a contract
//! that is `@mixin` the query builder, and both builders declare `paginate()`. At runtime the
//! relation's `__call` hands the call to the Eloquent builder, whose page says which model it holds;
//! the query builder's says nothing. The contract's was reached first, through what the relation
//! inherits, and filled the slot -- so every relation's page was of nothing.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_relations_own_mixin_answers_a_member_a_contracts_mixin_also_forwards() {
    check_view(
        r#"
//- /vendor/Orm.php
<?php
namespace Orm;

/** @template TItem */
class Page {}

/** @template TModel */
class Builder
{
    /** @return Page<TModel> */
    public function paginate() {}
}

class QueryBuilder
{
    /** @return Page<\stdClass> */
    public function paginate() {}
}

/** @mixin QueryBuilder */
interface QueryContract {}

/**
 * @template TRelated
 * @mixin Builder<TRelated>
 */
abstract class Relation implements QueryContract
{
    public function __call(string $method, array $arguments): mixed {}
}

/**
 * @template TRelated
 * @extends Relation<TRelated>
 */
class HasMany extends Relation {}

//- /src/User.php
<?php
namespace App;

class Post {}

class User
{
    /** @return \Orm\HasMany<Post> */
    public function posts() {}
}

function run(User $user): void
{
    $page = $user->posts()->paginate();
    $p@page@age;
}
"#,
        &[Query::variable_type("a relation's page", "page")],
        expect![[r#"
            a relation's page
            - Page<Post>
        "#]],
    );
}
