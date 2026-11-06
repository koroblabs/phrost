//! A template filled through a `@mixin` the class writes.
//!
//! Eloquent's shape: `Relation` writes `@mixin Builder<TRelatedModel>`, the builder `@use
//! BuildsQueries<TModel>`, and `first()` is the trait's `@return TValue|null` -- so on a
//! `HasMany<Post, User>` it is a `?Post`, two tags from the relation. A method returning `$this`
//! still answers the relation, which is what Laravel's decorated forwarding hands back. And the
//! relation's own mixin is read before one an interface above it writes: Laravel's builder
//! contract is `@mixin` the query builder, which fills the same trait with `stdClass`.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

/** @template TValue */
trait BuildsQueries
{
    /** @return TValue|null */
    public function first() {}
}

/** @template TModel */
class Builder
{
    /** @use BuildsQueries<TModel> */
    use BuildsQueries;

    /** @return $this */
    public function where(string $column) {}
}

class Row {}

/** The query builder beneath, which fills the same trait with rows. */
class QueryBuilder
{
    /** @use BuildsQueries<Row> */
    use BuildsQueries;
}

/** @mixin QueryBuilder */
interface QueryContract {}

/**
 * @template TRelated
 * @template TDeclaring
 * @mixin Builder<TRelated>
 */
abstract class Relation implements QueryContract
{
    public function __call(string $method, array $arguments): mixed {}
}

/**
 * @template TRelated
 * @template TDeclaring
 * @extends Relation<TRelated, TDeclaring>
 */
class HasMany extends Relation {}

//- /src/User.php
<?php
namespace App;

class Post {}

class User
{
    /** @return \Orm\HasMany<Post, User> */
    public function posts() {}
}

function run(User $user): void
{
    $f@first@irst = $user->posts()->first();
    $w@where@here = $user->posts()->where('title');
    $c@chained@hained = $user->posts()->where('title')->first();
}
"#;

#[test]
fn a_mixins_template_is_what_the_class_wrote_for_it() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("a trait member of the mixin, two tags away", "first"),
            Query::variable_type("$this through the mixin, which is the relation", "where"),
            Query::variable_type("and the chain after it", "chained"),
        ],
        expect![[r#"
            a trait member of the mixin, two tags away
            - ?Post
            $this through the mixin, which is the relation
            - HasMany<Post, User>
            and the chain after it
            - ?Post
        "#]],
    );
}
