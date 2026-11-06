//! Laravel's dynamic wheres offered in a list: `whereTitle` for the `title` a model states.
//!
//! The query builder's `__call` answers any `where` + word, so the list is the words the project wrote
//! down -- the model's stated columns -- on the model and on a builder for it, and nothing on a
//! builder that says no model.

use expect_test::expect;
use phrost_harness::{StatedKeyRule, StatedKind, StatedPropertiesRule};
use phrost_model::BuiltinTy;

use super::{Query, check_with};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

class Query
{
    public function __call(string $name, array $arguments): mixed {}
}

/**
 * @template TModel
 * @mixin Query
 */
class Builder
{
    public function __call(string $name, array $arguments): mixed {}
}

abstract class Model
{
    public static function __callStatic(string $name, array $arguments): mixed {}
    public function __call(string $name, array $arguments): mixed {}
    public function __get(string $key): mixed {}

    /** @return Builder<static> */
    public static function query() {}
}

//- /src/Post.php
<?php
namespace App;

use Orm\Model;

class Post extends Model
{
    protected $fillable = ['title'];

    protected $casts = ['published_at' => 'boolean'];
}

//- /src/OnTheModel.php
<?php
namespace App;

function onTheModel(): void
{
    Post::wh@static@
}

//- /src/OnTheBuilder.php
<?php
namespace App;

function onTheBuilder(): void
{
    Post::query()->whereP@builder@
}

//- /src/OnAQuery.php
<?php
namespace App;

function onAQuery(\Orm\Query $query): void
{
    $query->wh@query@
}
"#;

#[test]
fn a_where_is_offered_for_every_column_a_model_states() {
    check_with(
        FIXTURE,
        &[
            Query::completion("on the model", "static"),
            Query::completion("on a builder for it", "builder"),
            Query::completion("on a builder that names no model", "query"),
        ],
        |world| {
            world.set_kept_defaults(&["casts", "fillable"], &[]);
            world.add_implied_mixin("Orm\\Model", "Orm\\Builder", true);
            world
                .contracts_mut()
                .bind_fluent_prefix("Orm\\Query", "where");
            world
                .contracts_mut()
                .bind_stated_properties(StatedPropertiesRule {
                    class: "Orm\\Model".into(),
                    typed: vec!["casts".into()],
                    listed: vec!["fillable".into()],
                    listed_by_attribute: Vec::new(),
                    spellings: vec![("boolean".into(), StatedKind::Builtin(BuiltinTy::Bool))],
                    wrapping: Vec::new(),
                    classes: Vec::new(),
                    custom: None,
                    implicit: Vec::new(),
                    key: Some(StatedKeyRule {
                        name: "id".into(),
                        kind: StatedKind::Builtin(BuiltinTy::Int),
                        unless: Vec::new(),
                    }),
                    nullable: true,
                });
        },
        expect![[r#"
            on the model
            - whereTitle [method]  Post::$title  writes `(|);`
            - wherePublishedAt [method]  Post::$published_at  writes `(|);`
            - whereId [method]  Post::$id  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `Post::whereTitle`
            on a builder for it
            - wherePublishedAt [method]  Post::$published_at  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `Post::query()->wherePublishedAt`
            on a builder that names no model
            - (nothing offered)
        "#]],
    );
}
