//! Eloquent's local scopes, reached from a cursor: goto and completion on `Post::published()` and on
//! `Post::query()->published()`.
//!
//! The model's side answered goto and the builder's did not: a location is asked of a class and a
//! name, and `Builder` says which model it is for only in its type argument. The cursor's target is
//! built where the receiver is in hand, so the model is named there. And neither side offered the
//! name in a list -- `scopePublished` was offered, which is the one spelling nobody calls.
//!
//! A static `@method` the model carries that returns its builder is reached the same way --
//! `SoftDeletes`' `withTrashed()` -- and goto lands on the tag. And both are reached through a
//! relation, which forwards to the builder for its related model.

use expect_test::expect;

use super::{Query, check_with};

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
}

/** @method static Builder<static> withTrashed(bool $withTrashed = true) */
trait SoftDeletes {}

/**
 * @template TRelated
 * @mixin Builder<TRelated>
 */
class HasMany
{
    public function __call(string $name, array $arguments): mixed {}
}

//- /src/Post.php
<?php
namespace App;

use Orm\Model;

class Post extends Model
{
    use \Orm\SoftDeletes;

    public function scopePublished($query) {}

    public function scopeOfType($query, string $type, int $limit = 10) {}

    #[Scope]
    protected function popular($query): void {}

    /** @return \Orm\HasMany<Post> */
    public function related() {}
}

//- /src/Run.php
<?php
namespace App;

function run(): void
{
    Post::publ@model@ished();
    Post::query()->publ@builder@ished();
    Post::query()->popu@marked@lar();
    Post::query()->with@tagged@Trashed();
    (new Post())->related()->publ@relation@ished();
}

//- /src/OnTheBuilder.php
<?php
namespace App;

function onTheBuilder(): void
{
    Post::query()->p@typed@
}

//- /src/Tagged.php
<?php
namespace App;

function tagged(): void
{
    Post::query()->wi@tagcomp@
}

//- /src/OnARelation.php
<?php
namespace App;

function onARelation(Post $post): void
{
    $post->related()->with@relcomp@
}

//- /src/OnTheModel.php
<?php
namespace App;

function onTheModel(): void
{
    Post::of@static@
}
"#;

#[test]
fn a_scope_is_reached_from_the_model_and_from_a_builder_for_it() {
    check_with(
        FIXTURE,
        &[
            Query::goto("on the model", "model"),
            Query::goto("on a builder for it", "builder"),
            Query::goto("marked, on a builder", "marked"),
            Query::goto("tagged, on a builder", "tagged"),
            Query::goto("on a relation", "relation"),
            Query::completion("offered on a builder", "typed"),
            Query::completion("offered on the model", "static"),
            Query::completion("a tag, offered on a builder", "tagcomp"),
            Query::completion("a tag, offered on a relation", "relcomp"),
        ],
        |world| {
            world.set_keyed_attributes(&["Scope"], &[]);
            world
                .contracts_mut()
                .bind_prefixed_method("Orm\\Model", "scope", "Orm\\Builder", true);
        },
        expect![[r#"
            on the model
            - method App\Post::scopePublished @ src/Post.php 10:21-10:35 (full 10:5-10:46)
            on a builder for it
            - method App\Post::scopePublished @ src/Post.php 10:21-10:35 (full 10:5-10:46)
            marked, on a builder
            - method App\Post::popular @ src/Post.php 15:24-15:31 (full 14:5-15:48)
            tagged, on a builder
            - method Orm\SoftDeletes::withTrashed @ vendor/Orm.php 19:36-19:47 (full 19:13-19:73)
            on a relation
            - method App\Post::scopePublished @ src/Post.php 10:21-10:35 (full 10:5-10:46)
            offered on a builder
            - popular [method]  Post::popular  writes `();`
            - published [method]  Post::scopePublished  writes `();`
            - ... and the client must ask again
            - accepting the first leaves `Post::query()->popular`
            offered on the model
            - ofType [method]  Post::scopeOfType  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `Post::ofType`
            a tag, offered on a builder
            - withTrashed [method]  Post::withTrashed  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `Post::query()->withTrashed`
            a tag, offered on a relation
            - withTrashed [method]  Post::withTrashed  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `$post->related()->withTrashed`
        "#]],
    );
}
