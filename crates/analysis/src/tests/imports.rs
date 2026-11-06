//! A `use` line nothing in the file names.
//!
//! The check itself is a thin wrapper over
//! [`unused_imports`](phrost_view::IndexView::unused_imports) -- see that module's tests for the
//! exhaustive list of what counts as "used". These tests are only about the wiring: severity,
//! code, message, and that it sits correctly next to the rest of the checks.
//!
//! Every imported class here is declared for real in its own file. Naming one that does not exist
//! would also trip `undefined-type`, which is a different check for a different reason -- and
//! these tests are about `unused-import` alone.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_import_nothing_names_is_reported() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App\Support;

class Repo {}

//- /src/Dangling.php
<?php
namespace App\Support;

class Dangling {}

//- /app.php
<?php
namespace App;

use App\Support\Repo;
use App\Support\Dangling;

class Service
{
    public function run(Repo $repo): void {}
}
"#,
        expect![[r#"
            5:5-5:25 warning [unused-import] Unused import App\Support\Dangling
        "#]],
    );
}

#[test]
fn each_symbol_table_is_checked_on_its_own() {
    check(
        r#"
//- /src/Thing.php
<?php
namespace App\Support;

class Thing {}

//- /app.php
<?php
namespace App;

use App\Support\Thing;
use function App\Support\thing;
use const App\Support\THING;
"#,
        expect![[r#"
            4:5-4:22 warning [unused-import] Unused import App\Support\Thing
            5:14-5:31 warning [unused-import] Unused import App\Support\thing
            6:11-6:28 warning [unused-import] Unused import App\Support\THING
        "#]],
    );
}

#[test]
fn a_used_import_reports_nothing() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App\Support;

class Repo {}

//- /app.php
<?php
namespace App;

use App\Support\Repo;

class Service
{
    public function run(Repo $repo): void {}
}
"#,
        expect![""],
    );
}

#[test]
fn an_unused_import_and_an_undefined_name_are_reported_once_each() {
    // The two checks watch different things -- unused-import asks whether the file names its own
    // `use` lines, undefined-type asks whether a written name resolves -- and a fixture that
    // exercises both must not see either one twice.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App\Support;

class Repo {}

//- /src/Dangling.php
<?php
namespace App\Support;

class Dangling {}

//- /app.php
<?php
namespace App;

use App\Support\Repo;
use App\Support\Dangling;

class Service
{
    public function run(Missing $missing): void {}
}
"#,
        expect![[r#"
            4:5-4:21 warning [unused-import] Unused import App\Support\Repo
            5:5-5:25 warning [unused-import] Unused import App\Support\Dangling
            9:25-9:32 error [undefined-type] Undefined type Missing
        "#]],
    );
}

#[test]
fn nothing_is_reported_for_a_file_that_did_not_parse() {
    // The projection declines to answer at all once the file has a parse error, so an import used
    // nowhere above the break still is not reported -- the same silence as `unused-variable` under
    // a break, for the same reason: the statement that would have used it may be the one recovery
    // dropped.
    check(
        r#"
//- /src/Dangling.php
<?php
namespace App\Support;

class Dangling {}

//- /app.php
<?php
namespace App;

use App\Support\Dangling;

class Service
{
    public function run(): void
    {
        $this->
    }
}
"#,
        expect![[r#"
            11:5-11:6 error [syntax] Expected one of `Variable`, `DollarLeftBrace`, `Dollar`, `LeftBrace`, `Identifier`, found `RightBrace`
            11:5-11:6 error [syntax] Expected one of `Semicolon`, `CloseTag`, found `RightBrace`
        "#]],
    );
}
