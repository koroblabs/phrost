//! A global name a framework's loader aliases, reached from a cursor: Laravel's `use DB;`.
//!
//! Goto lands on the class the name is an alias of, hover describes it, and neither the import nor
//! a use of it is a type nothing declares.

use expect_test::expect;

use super::{Query, check_with};

const FIXTURE: &str = r#"
//- /vendor/orm/Facades/Database.php
<?php
namespace Orm\Facades;

/** The database facade. */
class Database
{
    public static function table(string $name): void {}
}

//- /src/Repo.php
<?php
namespace App;

use D@import@B;

function run(): void
{
    D@used@B::table('users');
}
"#;

#[test]
fn an_alias_goes_to_and_describes_the_class_it_names() {
    check_with(
        FIXTURE,
        &[
            Query::goto("from the import", "import"),
            Query::goto("from a use", "used"),
            Query::hover("described", "used"),
        ],
        |world| world.add_class_alias("DB", "Orm\\Facades\\Database"),
        expect![[r#"
            from the import
            - class Orm\Facades\Database @ vendor/orm/Facades/Database.php 5:7-5:15 (full 5:1-8:2)
            from a use
            - class Orm\Facades\Database @ vendor/orm/Facades/Database.php 5:7-5:15 (full 5:1-8:2)
            described
            - code: <?php
            namespace Orm\Facades;

            class Database
            - ---
            - doc: The database facade.
        "#]],
    );
}

#[test]
fn an_alias_is_not_an_undefined_type() {
    super::diagnostics::check_framework(
        FIXTURE,
        |world| world.add_class_alias("DB", "Orm\\Facades\\Database"),
        expect![""],
    );
}
