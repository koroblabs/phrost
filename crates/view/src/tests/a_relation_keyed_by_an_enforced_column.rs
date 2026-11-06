//! A relation reading a key in the model's own row is never empty where the key cannot be.
//!
//! Laravel's `BelongsTo` loads what `{method}_id` points at, and declares `TRelated|null`. A column
//! that cannot be null and is a foreign key the database enforces always points at a row that
//! exists, so `$user->account` over `foreignIdFor(Account::class)->constrained()` is an `Account`.
//! A nullable key, an unconstrained one, and a key no migration defines keep the declared null.

use expect_test::expect;

use phrost_harness::World;
use phrost_index::schema::WrittenSchemaRule;
use phrost_model::{
    BuiltinTy,
    schema::{BuilderCall, ColumnChange},
};
use phrost_testkit::FixtureSpec;

use super::{Query, check_world};

/// `integer('x')` with `->nullable()` and `->constrained()`, which is all a key needs.
fn columns(call: &BuilderCall) -> Vec<ColumnChange> {
    let has = |wanted: &str| call.then.iter().any(|(method, _)| method == wanted);
    match (
        call.method.as_str(),
        call.arguments.first().and_then(|written| written.first()),
    ) {
        ("integer", Some(name)) => vec![ColumnChange::Defined {
            name: name.clone(),
            holds: BuiltinTy::Int,
            nullable: has("nullable"),
            constrained: has("constrained"),
        }],
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

    /**
     * @template TRelated
     * @param class-string<TRelated> $related
     * @return BelongsTo<TRelated>
     */
    public function belongsTo(string $related) {}
}

abstract class Relation {}

/** @template TRelated */
class BelongsTo extends Relation
{
    /** @return TRelated|null */
    public function getResults() {}
}

final class Schema
{
    public static function create(string $table, \Closure $build): void {}
}

//- /database/migrations/2020_01_01_create_users.php
<?php
use Orm\Schema;

return new class {
    public function up(): void
    {
        Schema::create('users', function ($table) {
            $table->integer('account_id')->constrained();
            $table->integer('owner_id');
            $table->integer('editor_id')->nullable()->constrained();
        });
    }
};

//- /src/User.php
<?php
namespace App;

class Account {}

class User extends \Orm\Model
{
    /** @return \Orm\BelongsTo<Account> */
    public function account() { return $this->belongsTo(Account::class); }

    /** @return \Orm\BelongsTo<Account> */
    public function owner() { return $this->belongsTo(Account::class); }

    /** @return \Orm\BelongsTo<Account> */
    public function editor() { return $this->belongsTo(Account::class); }

    /** @return \Orm\BelongsTo<Account> */
    public function reviewer() { return $this->belongsTo(Account::class); }
}

function run(User $user): void
{
    $a@account@ccount = $user->account;
    $o@owner@wner = $user->owner;
    $e@editor@ditor = $user->editor;
    $r@reviewer@eviewer = $user->reviewer;
}
"#;

fn rules(world: &mut World) {
    world.contracts_mut().bind_method_property(
        "Orm\\Model",
        "Orm\\Relation",
        "getResults",
        None,
        &[("Orm\\BelongsTo", "_id")],
    );
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
fn a_relation_is_empty_only_where_its_key_can_be() {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build_with(&spec, rules);
    check_world(
        &spec,
        &world,
        &[
            Query::variable_type("a key that cannot be null, enforced", "account"),
            Query::variable_type("one the database does not enforce", "owner"),
            Query::variable_type("one that can be null", "editor"),
            Query::variable_type("one no migration defines", "reviewer"),
        ],
        expect![[r#"
            a key that cannot be null, enforced
            - Account
            one the database does not enforce
            - ?Account
            one that can be null
            - ?Account
            one no migration defines
            - ?Account
        "#]],
    );
}
