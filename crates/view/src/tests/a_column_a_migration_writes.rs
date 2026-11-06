//! A model's columns, as the project's migrations write its table.
//!
//! Nothing declares `title` on a model; `__get` reads the row, and the migration that created the
//! table is the only statement of what is in it. So a column nothing on the class mentions is a
//! property holding what the database hands back, a listed one says what it holds at last, and a
//! cast keeps its type and takes the column's null -- `datetime` on a column that cannot be null is
//! a date, not a date or nothing. A model naming its table reads that one; any other reads the one
//! its short name makes.

use expect_test::expect;

use phrost_harness::World;
use phrost_index::{StatedKind, StatedPropertiesRule, schema::WrittenSchemaRule};
use phrost_model::{
    BuiltinTy,
    schema::{BuilderCall, ColumnChange},
};
use phrost_testkit::FixtureSpec;

use super::{Query, check_world};

/// `string('x')`, `integer('x')`, `->nullable()` and `timestamps()` -- enough of a builder to see
/// what a column holds reach the model.
fn columns(call: &BuilderCall) -> Vec<ColumnChange> {
    let nullable = call.then.iter().any(|(method, _)| method == "nullable");
    let first = call.arguments.first().and_then(|written| written.first());
    let constrained = call.then.iter().any(|(method, _)| method == "constrained");
    let defined = |name: &str, holds, nullable| ColumnChange::Defined {
        name: name.to_owned(),
        holds,
        nullable,
        constrained,
    };
    match call.method.as_str() {
        "string" => first
            .map(|name| defined(name, BuiltinTy::String, nullable))
            .into_iter()
            .collect(),
        "integer" => first
            .map(|name| defined(name, BuiltinTy::Int, nullable))
            .into_iter()
            .collect(),
        "timestamps" => vec![
            defined("created_at", BuiltinTy::String, true),
            defined("updated_at", BuiltinTy::String, true),
        ],
        _ => Vec::new(),
    }
}

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

abstract class Model
{
    public function __get(string $key): mixed {}
}

final class Date {}

final class Schema
{
    public static function create(string $table, \Closure $build): void {}
}

//- /database/migrations/2020_01_01_create_posts.php
<?php
use Orm\Schema;

return new class {
    public function up(): void
    {
        Schema::create('posts', function ($table) {
            $table->integer('id');
            $table->string('title');
            $table->string('subtitle')->nullable();
            $table->string('published_at');
            $table->string('edited_at')->nullable();
            $table->integer('votes');
            $table->timestamps();
        });
        Schema::create('archive', function ($table) {
            $table->string('reason');
        });
        Schema::create('tokens', function ($table) {
            $table->string('id');
        });
    }
};

//- /src/Post.php
<?php
namespace App;

class Post extends \Orm\Model
{
    protected $fillable = ['votes'];

    protected $casts = ['published_at' => 'datetime', 'edited_at' => 'datetime'];
}

class Archived extends \Orm\Model
{
    protected $table = 'archive';
}

class Unknown extends \Orm\Model {}

class Token extends \Orm\Model
{
    public function getKeyType(): string { return 'string'; }
}

function run(Post $post, Archived $archived, Unknown $unknown, Token $token): void
{
    $t@title@itle = $post->title;
    $s@subtitle@ubtitle = $post->subtitle;
    $p@published@ublished = $post->published_at;
    $e@edited@dited = $post->edited_at;
    $v@votes@otes = $post->votes;
    $c@created@reated = $post->created_at;
    $i@id@d = $post->id;
    $r@reason@eason = $archived->reason;
    $n@nothing@othing = $unknown->title;
    $k@key@ey = $token->id;
}
"#;

fn rules(world: &mut World) {
    world.set_kept_defaults(&["casts", "fillable", "table"], &[]);
    world
        .contracts_mut()
        .bind_stated_properties(StatedPropertiesRule {
            class: "Orm\\Model".into(),
            typed: vec!["casts".into()],
            listed: vec!["fillable".into()],
            listed_by_attribute: Vec::new(),
            spellings: vec![("datetime".into(), StatedKind::Class("Orm\\Date".into()))],
            wrapping: Vec::new(),
            classes: Vec::new(),
            custom: None,
            implicit: vec![("created_at".into(), StatedKind::Class("Orm\\Date".into()))],
            key: Some(phrost_index::StatedKeyRule {
                name: "id".into(),
                kind: StatedKind::Builtin(BuiltinTy::Int),
                unless: vec!["getKeyType".into()],
            }),
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
}

#[test]
fn a_model_reads_the_columns_its_table_has() {
    let spec = FixtureSpec::parse(FIXTURE);
    // Before the files, because lowering keeps only the defaults it was told to.
    let world = World::build_with(&spec, rules);
    check_world(
        &spec,
        &world,
        &[
            Query::variable_type("a column nothing on the class mentions", "title"),
            Query::variable_type("one that can be null", "subtitle"),
            Query::variable_type("a cast on a column that cannot be null", "published"),
            Query::variable_type("a cast on one that can", "edited"),
            Query::variable_type("a listed name, which says what it holds at last", "votes"),
            Query::variable_type("what every model has, on a nullable column", "created"),
            Query::variable_type("the key, on a column that cannot be null", "id"),
            Query::variable_type("a model naming its table", "reason"),
            Query::variable_type("a model whose table no migration writes", "nothing"),
            Query::variable_type("a key the class changed, which the column says", "key"),
            Query::property_goto("goto, a column", "App\\Post", "title"),
            Query::property_goto("goto, the key", "App\\Post", "id"),
            Query::property_goto(
                "goto, a cast still goes to the cast",
                "App\\Post",
                "published_at",
            ),
        ],
        expect![[r#"
            a column nothing on the class mentions
            - string
            one that can be null
            - ?string
            a cast on a column that cannot be null
            - Date
            a cast on one that can
            - ?Date
            a listed name, which says what it holds at last
            - int
            what every model has, on a nullable column
            - ?Date
            the key, on a column that cannot be null
            - int
            a model naming its table
            - string
            a model whose table no migration writes
            - mixed
            a key the class changed, which the column says
            - string
            goto, a column
            - property App\Post::title @ database/migrations/2020_01_01_create_posts.php 9:29-9:34
            goto, the key
            - property App\Post::id @ database/migrations/2020_01_01_create_posts.php 8:30-8:32
            goto, a cast still goes to the cast
            - property App\Post::published_at @ src/Post.php 8:26-8:38
        "#]],
    );
}
