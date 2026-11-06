//! A global name a framework's loader `class_alias`es at start, which no file declares.
//!
//! Laravel's `use DB;` and a bare `DB::table()`: the alias loader makes `DB` the facade the first
//! time it is asked for. Answered by the class it names, as if declared under both -- and a name a
//! file does declare is the declaration's, since PHP's `class_alias` refuses a name already taken.

use expect_test::expect;

use super::{Query, check_view_ingested_with};

#[test]
fn an_alias_answers_as_the_class_it_names() {
    check_view_ingested_with(
        r#"
//- /vendor/orm/Facades/Database.php
<?php
namespace Orm\Facades;

/** @method static \Orm\Query table(string $name) */
class Database {}

//- /vendor/orm/Query.php
<?php
namespace Orm;

class Query {}

//- /src/Taken.php
<?php
class Taken {}

//- /src/Repo.php
<?php
namespace App;

use DB;

function run(): void
{
    $q = DB::table('users');
    $@q@q;
    $t = new \Taken();
    $@t@t;
}
"#,
        &[
            Query::variable_type("a call through it", "q"),
            Query::variable_type("a name a file declares is the declaration's", "t"),
        ],
        |world| {
            world.add_class_alias("DB", "Orm\\Facades\\Database");
            world.add_class_alias("Taken", "Orm\\Query");
        },
        expect![[r#"
            a call through it
            - Query
            a name a file declares is the declaration's
            - Taken
        "#]],
    );
}
