//! A property no declaration writes, read through the method of the same name.
//!
//! Eloquent's relations: `$post->comments` is `$post->comments()->getResults()`, because the model's
//! `__get` finds the method, sees a relation, and loads it. The framework states the rule and the
//! project's own method says what comes back -- in its docblock, or, where the hint names the
//! relation bare as Laravel's own documentation writes it, in the call its body returns.

use expect_test::expect;

use super::{Query, check_view_with};

const FIXTURE: &str = r#"
//- /vendor/Model.php
<?php
namespace Orm;

abstract class Model
{
    public function __get(string $key): mixed {}

    /** @return Builder */
    public function newQuery() {}

    /**
     * @template TRelated
     * @param class-string<TRelated> $related
     * @return BelongsTo<TRelated>
     */
    public function belongsTo(string $related) {}
}

class Builder {}

/**
 * @template TRelated
 * @template TResult
 */
abstract class Relation
{
    /** @return TResult */
    public function getResults() {}
}

/** @template TValue */
class Collection
{
    /** @return TValue */
    public function first() {}
}

/**
 * @template TRelated
 * @extends Relation<TRelated, Collection<TRelated>>
 */
class HasMany extends Relation {}

/**
 * @template TRelated
 * @extends Relation<TRelated, ?TRelated>
 */
class BelongsTo extends Relation {}

//- /src/Post.php
<?php
namespace App;

use Orm\BelongsTo;
use Orm\HasMany;
use Orm\Model;

class User extends Model {}
class Comment extends Model {}

class Post extends Model
{
    /** @return HasMany<Comment> */
    public function comments(): HasMany {}

    /** @return BelongsTo<User> */
    public function author(): BelongsTo {}

    public function title(): string {}

    public function editor(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }
}

function run(Post $post): void
{
    $c@comments@omments = $post->comments;
    $a@author@uthor = $post->author;
    $t@title@itle = $post->title;
    $e@editor@ditor = $post->editor;
    $n@counted@umber = $post->comments_count;
    $x@not_counted@x = $post->title_count;
}
"#;

#[test]
fn a_relation_read_as_a_property_is_what_the_relation_loads() {
    check_view_with(
        FIXTURE,
        &[
            Query::variable_type("a many-relation", "comments"),
            Query::variable_type("a single one, which may find nothing", "author"),
            Query::variable_type("a method that is no relation", "title"),
            Query::variable_type("a bare hint the body fills in", "editor"),
            Query::variable_type(
                "a relation's count, which a query may not have taken",
                "counted",
            ),
            Query::variable_type("a method that is no relation counts nothing", "not_counted"),
            Query::property_goto("goto, a count", "App\\Post", "comments_count"),
        ],
        |world| {
            world.contracts_mut().bind_method_property(
                "Orm\\Model",
                "Orm\\Relation",
                "getResults",
                Some("_count"),
                &[],
            );
        },
        expect![[r#"
            a many-relation
            - Collection<Comment>
            a single one, which may find nothing
            - ?User
            a method that is no relation
            - mixed
            a bare hint the body fills in
            - ?User
            a relation's count, which a query may not have taken
            - ?int
            a method that is no relation counts nothing
            - mixed
            goto, a count
            - method App\Post::comments @ src/Post.php 14:21-14:29
        "#]],
    );
}
