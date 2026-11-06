//! A static `@method` on a model that returns its builder is a method of that builder too.
//!
//! `SoftDeletes` writes `@method static Builder<static> withTrashed()`, and `Post::withTrashed()`
//! reaches it through `__callStatic`, which makes a fresh builder and calls the name on that -- so a
//! builder for the model answers the same name, and hands itself back. Laravel registers the
//! method as a macro the builder holds at runtime, which nothing reads; the tag is what states it.
//! `$book->pages()->withTrashed()->get()` is how every soft-deleting relation is read, and the
//! chain ended at the macro.

use expect_test::expect;

use super::{Query, check_view_with};

const FIXTURE: &str = r#"
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

    /** @return ?TModel */
    public function first() {}
}

/**
 * @method static Builder<static> withTrashed(bool $withTrashed = true)
 * @method static static restoreOrCreate(array $attributes = [])
 */
trait SoftDeletes
{
    public function restore(): bool {}
}

//- /src/Post.php
<?php
namespace App;

use Orm\Model;
use Orm\SoftDeletes;

class Post extends Model
{
    use SoftDeletes;
}

class Note extends Model {}

function run(): void
{
    $s@statically@tatically = Post::withTrashed();
    $b@builder@uilder = Post::query()->withTrashed();
    $f@first@irst = Post::query()->withTrashed()->first();
    $o@other@ther = Post::query()->restoreOrCreate();
    $n@nothing@othing = Note::query()->withTrashed();
}
"#;

#[test]
fn a_builder_answers_what_its_model_tags_as_returning_it() {
    check_view_with(
        FIXTURE,
        &[
            Query::variable_type("on the model, the tag", "statically"),
            Query::variable_type("on a builder for it", "builder"),
            Query::variable_type("and the chain goes on", "first"),
            Query::variable_type("a tag returning something else", "other"),
            Query::variable_type("a model without the tag", "nothing"),
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
            on the model, the tag
            - Builder<Post>
            on a builder for it
            - Builder<Post>
            and the chain goes on
            - ?Post
            a tag returning something else
            - mixed
            a model without the tag
            - mixed
        "#]],
    );
}
