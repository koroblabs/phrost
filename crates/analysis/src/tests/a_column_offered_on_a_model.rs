//! A model's columns in a list: the properties its table has, and a dynamic where for each.
//!
//! The migration is the only statement of which columns there are, so a model that states nothing
//! still has every one -- each with what it holds -- and the query builder's `__call` answers a
//! `where` for each of them, which is the half of a list a person writing a query reaches for.

use expect_test::expect;
use phrost_harness::{StatedPropertiesRule, WrittenSchemaRule};
use phrost_model::{
    BuiltinTy,
    schema::{BuilderCall, ColumnChange},
};

use super::{Query, check_with};

/// `string('x')` and `integer('x')`, with `->nullable()`.
fn columns(call: &BuilderCall) -> Vec<ColumnChange> {
    let holds = match call.method.as_str() {
        "string" => BuiltinTy::String,
        "integer" => BuiltinTy::Int,
        _ => return Vec::new(),
    };
    call.arguments
        .first()
        .and_then(|written| written.first())
        .map(|name| ColumnChange::Defined {
            name: name.clone(),
            holds,
            nullable: call.then.iter().any(|(method, _)| method == "nullable"),
            constrained: call.then.iter().any(|(method, _)| method == "constrained"),
        })
        .into_iter()
        .collect()
}

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
}

final class Schema
{
    public static function create(string $table, \Closure $build): void {}
}

//- /database/migrations/2020_01_01_posts.php
<?php
return new class {
    public function up(): void
    {
        \Orm\Schema::create('posts', function ($table) {
            $table->string('title');
            $table->integer('votes')->nullable();
        });
    }
};

//- /src/Post.php
<?php
namespace App;

class Post extends \Orm\Model {}

//- /src/Use.php
<?php
namespace App;

function onTheModel(Post $post): void
{
    $post->vo@property@
}

function aWhere(): void
{
    Post::whereT@where@
}
"#;

#[test]
fn every_column_is_offered_as_a_property_and_a_where() {
    check_with(
        FIXTURE,
        &[
            Query::completion("a property", "property"),
            Query::completion("a where", "where"),
        ],
        |world| {
            world.add_implied_mixin("Orm\\Model", "Orm\\Builder", true);
            world
                .contracts_mut()
                .bind_fluent_prefix("Orm\\Query", "where");
            world
                .contracts_mut()
                .bind_stated_properties(StatedPropertiesRule {
                    class: "Orm\\Model".into(),
                    typed: Vec::new(),
                    listed: Vec::new(),
                    listed_by_attribute: Vec::new(),
                    spellings: Vec::new(),
                    wrapping: Vec::new(),
                    classes: Vec::new(),
                    custom: None,
                    implicit: Vec::new(),
                    key: None,
                    nullable: true,
                });
            world
                .contracts_mut()
                .bind_written_schema(WrittenSchemaRule {
                    facade: "Orm\\Schema".into(),
                    create: vec!["create".into()],
                    alter: Vec::new(),
                    drop: Vec::new(),
                    rename: Vec::new(),
                    runs: "up".into(),
                    directory: "migrations".into(),
                    columns,
                    model: "Orm\\Model".into(),
                    table_property: "table".into(),
                    default_table: |short| format!("{}s", short.to_ascii_lowercase()),
                });
        },
        expect![[r#"
            a property
            - votes [property]  ?int
            - whereVotes [method]  Post::$votes  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `$post->votes`
            a where
            - whereTitle [method]  Post::$title  writes `(|);`
            - whereVotes [method]  Post::$votes  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `Post::whereTitle`
        "#]],
    );
}
