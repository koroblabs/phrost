//! A scope or a builder tag called on a relation: `$post->comments()->published()`.
//!
//! `Relation` forwards what it does not declare to its builder -- `@mixin Builder<TRelatedModel>`
//! -- and hands itself back where the builder handed back itself. The builder answered a scope only
//! when it was the receiver, so through a relation the call reached `__call` and was `mixed`, and
//! `$book->pages()->withTrashed()->get()` ended there.

use expect_test::expect;

use super::{Query, check_view_with};

#[test]
fn a_relation_answers_what_its_builder_does_and_hands_itself_back() {
    check_view_with(
        r#"
//- /vendor/Orm.php
<?php
namespace Orm;

abstract class Model
{
    public static function __callStatic(string $name, array $arguments): mixed {}
    public function __call(string $name, array $arguments): mixed {}

    /** @return Builder<static> */
    public static function query() {}
}

/** @template TModel */
class Builder
{
    public function __call(string $name, array $arguments): mixed {}

    /** @return $this */
    public function where(string $column) {}

    /** @return ?TModel */
    public function first() {}
}

/**
 * @template TRelated
 * @mixin Builder<TRelated>
 */
class HasMany
{
    public function __call(string $name, array $arguments): mixed {}
}

/** @method static Builder<static> withTrashed(bool $withTrashed = true) */
trait SoftDeletes {}

//- /src/Post.php
<?php
namespace App;

class Comment extends \Orm\Model
{
    use \Orm\SoftDeletes;

    public function scopePublished($query) {}
}

class Post extends \Orm\Model
{
    /** @return \Orm\HasMany<Comment> */
    public function comments() {}
}

function run(Post $post): void
{
    $w@where@here = $post->comments()->where('a');
    $s@scope@cope = $post->comments()->published();
    $t@tag@ag = $post->comments()->withTrashed();
    $f@first@irst = $post->comments()->withTrashed()->first();
}
"#,
        &[
            Query::variable_type("a builder method, for comparison", "where"),
            Query::variable_type("a scope of the related model", "scope"),
            Query::variable_type("a tag of the related model", "tag"),
            Query::variable_type("and the chain goes on", "first"),
        ],
        |world| {
            world.contracts_mut().bind_prefixed_method(
                "Orm\\Model",
                "scope",
                "Orm\\Builder",
                false,
            );
        },
        expect![[r#"
            a builder method, for comparison
            - HasMany<Comment>
            a scope of the related model
            - HasMany<Comment>
            a tag of the related model
            - HasMany<Comment>
            and the chain goes on
            - ?Comment
        "#]],
    );
}
