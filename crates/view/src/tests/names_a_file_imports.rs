//! The names a file's own `use` lines bind, on a completion page that is ordered and cut without
//! knowing about them -- see [`crate::candidates`].
//!
//! Called directly rather than through [`super::Query`], for the reason `candidates` gives: the
//! property is about `limit`, and the report was a page of two hundred names that did not hold the
//! one the file had imported.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;

use crate::{IndexView, NameKinds};

/// A dependency's class and function the file imports, four of the project's own names that sort
/// before them, a namesake nobody imported, and an import of something nothing declares.
const PROJECT: &str = r#"
//- /vendor/acme/src/Date.php
<?php
namespace Acme\Support;

class Date {}

function dated(): void {}

//- /vendor/other/src/Date.php
<?php
namespace Other;

class Date {}

//- /src/Dashboard.php
<?php
namespace App;

class Dashboard {}

class Dat {}

class DataRow {}

class DataTable {}

//- /src/Report.php
<?php
namespace App;

use Acme\Support\Date;
use Vendor\NotInstalled\Dated;
use function Acme\Support\dated;

class Report
{
    public function run(): void
    {
        @here@
    }
}
"#;

fn check(prefix: &str, kinds: NameKinds, limit: usize, expect: Expect) {
    let spec = FixtureSpec::parse(PROJECT);
    let world = World::build(&spec);
    let file = world.file_id("src/Report.php");
    let offset = spec.markers().position("here").offset;

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        view.candidates_with_prefix(offset, prefix, kinds, limit)
            .iter()
            .map(|candidate| format!("{:?} {}\n", candidate.kind, candidate.fqn))
            .collect::<String>()
    });
    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

/// The imported class leads the project's own names, which lead the namesake nobody imported.
#[test]
fn what_the_file_imports_comes_before_what_it_would_have_to_import() {
    check(
        "Da",
        NameKinds::ClassOnly,
        200,
        expect![[r#"
        Class Acme\Support\Date
        Class App\Dashboard
        Class App\Dat
        Class App\DataRow
        Class App\DataTable
        Class Other\Date
    "#]],
    );
}

/// A page too small for every match still holds the import: the cap is what cut it, on a real
/// project, at two hundred.
#[test]
fn an_import_is_on_the_page_however_small_the_page_is() {
    check(
        "Da",
        NameKinds::ClassOnly,
        2,
        expect![[r#"
        Class Acme\Support\Date
        Class App\Dashboard
    "#]],
    );
}

/// Each `use` line in the table it binds into: `use function` names a function and no class.
#[test]
fn a_function_import_is_a_function() {
    check(
        "Da",
        NameKinds::Any,
        200,
        expect![[r#"
        Class Acme\Support\Date
        Function Acme\Support\dated
        Class App\Dashboard
        Class App\Dat
        Class App\DataRow
        Class App\DataTable
        Class Other\Date
    "#]],
    );
}

/// What was typed in full still leads an import that only begins with it.
#[test]
fn the_name_typed_in_full_leads_an_import_that_only_begins_with_it() {
    check(
        "Dat",
        NameKinds::ClassOnly,
        200,
        expect![[r#"
        Class App\Dat
        Class Acme\Support\Date
        Class App\DataRow
        Class App\DataTable
        Class Other\Date
    "#]],
    );
}

/// And of two names typed in full, the imported one is first: the other has to be written out.
#[test]
fn of_two_names_typed_in_full_the_imported_one_leads() {
    check(
        "Date",
        NameKinds::ClassOnly,
        200,
        expect![[r#"
        Class Acme\Support\Date
        Class Other\Date
        Class App\DataTable
    "#]],
    );
}
