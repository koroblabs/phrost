//! A method called without the prefix it is declared under: Eloquent's local scopes.
//!
//! `scopePublished($query)` is called `Post::published()` and `$query->published()`, and hands back
//! the builder. Nothing declares `published`, so before the rule the chain died at the scope -- the
//! commonest line in a Laravel query. Laravel 12 writes the same thing as `#[Scope] protected
//! function popular($query)`, called under its own name: from outside the class PHP runs the
//! magic method, and inside it the declaration.

use expect_test::expect;

use phrost_harness::World;
use phrost_testkit::FixtureSpec;

use super::{Query, check_view_with, check_world};

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

//- /src/Post.php
<?php
namespace App;

use Orm\Model;

class Post extends Model
{
    public function scopePublished($query) {}

    #[Scope]
    protected function popular($query): void {}

    public function inside(): void
    {
        $i@inside@nside = $this->popular(Post::query());
    }
}

function run(): void
{
    $s@statically@tatically = Post::published();
    $b@builder@uilder = Post::query()->published();
    $f@first@irst = Post::query()->published()->first();
    $n@nothing@othing = Post::query()->drafts();
    $m@marked@arked = Post::popular()->first();
    $o@onbuilder@nbuilder = Post::query()->popular();
}
"#;

const QUERIES: &[Query] = &[
    Query::variable_type("on the model", "statically"),
    Query::variable_type("on a builder for it", "builder"),
    Query::variable_type("and the chain goes on", "first"),
    Query::variable_type("a name no scope has", "nothing"),
    Query::variable_type("marked, from outside, through the magic method", "marked"),
    Query::variable_type("marked, on a builder for the model", "onbuilder"),
    Query::variable_type("marked, inside the class, the declaration", "inside"),
];

/// Without the marker the rule says nothing about `#[Scope]`, and a protected method called from
/// outside is the magic method's, which says nothing the chain after it can use.
#[test]
fn a_scope_is_called_by_its_name_and_hands_back_the_builder() {
    check_view_with(
        FIXTURE,
        QUERIES,
        |world| {
            world.contracts_mut().bind_prefixed_method(
                "Orm\\Model",
                "scope",
                "Orm\\Builder",
                false,
            );
        },
        expect![[r#"
            on the model
            - Builder<Post>
            on a builder for it
            - Builder<Post>
            and the chain goes on
            - ?Post
            a name no scope has
            - mixed
            marked, from outside, through the magic method
            - (no type)
            marked, on a builder for the model
            - mixed
            marked, inside the class, the declaration
            - void
        "#]],
    );
}

#[test]
fn a_marked_method_is_called_the_same_way_under_its_own_name() {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build_with(&spec, |world| {
        // Before the files, because lowering marks what it was told to.
        world.set_keyed_attributes(&["Scope"], &[]);
        world
            .contracts_mut()
            .bind_prefixed_method("Orm\\Model", "scope", "Orm\\Builder", true);
    });
    check_world(
        &spec,
        &world,
        QUERIES,
        expect![[r#"
            on the model
            - Builder<Post>
            on a builder for it
            - Builder<Post>
            and the chain goes on
            - ?Post
            a name no scope has
            - mixed
            marked, from outside, through the magic method
            - ?Post
            marked, on a builder for the model
            - Builder<Post>
            marked, inside the class, the declaration
            - void
        "#]],
    );
}
