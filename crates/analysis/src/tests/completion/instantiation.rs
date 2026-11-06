//! What `new` offers, which is only what `new` can build.
//!
//! `new strlen()`, `new PHP_EOL`, `new Countable()` and `new SomeTrait()` are each a fatal error.
//! Offering them cost more than precision: with nothing typed yet the catalog is mostly functions
//! and constants, so the list's whole budget went on rows nobody could accept — measured on a real
//! project, two hundred of them before the first class.

use expect_test::expect;

use crate::tests::{Query, check};

/// One declaration of each kind, all answering to the same prefix, so a snapshot says which of them
/// survive a position rather than only that something was offered.
const KIT: &str = r#"
//- /src/kit.php
<?php
namespace App;

class Repo {}

interface Repository {}

trait Repeatable {}

enum Repeated {}

function repay(): void {}

const REPAID = 1;
"#;

/// The kit, plus a file asking one question of it.
fn asking(question: &str) -> String {
    format!("{KIT}{question}")
}

#[test]
fn only_a_class_is_offered_after_new() {
    check(
        &asking(
            r#"
//- /src/Client.php
<?php
namespace App;

class Client
{
    public function run(): void
    {
        new Rep@here@
    }
}
"#,
        ),
        &[Query::completion("after new", "here")],
        expect![[r#"
            after new
            - Repo [class]  App\Repo
            - ... and the client must ask again
            - accepting the first leaves `new Repo`
        "#]],
    );
}

/// A vendor class that sorts alphabetically before the project's own -- the shape of the real
/// regression: with nothing typed yet, every class in the catalog matches, and a plain alphabetical
/// list lets the standard library (or, here, one dependency standing in for it) fill the page
/// before a project's own class is ever reached.
#[test]
fn a_project_class_is_offered_before_a_vendor_class_that_sorts_first() {
    check(
        r#"
//- /vendor/acme/src/AaaWidget.php
<?php
namespace Acme;

class AaaWidget {}

//- /src/Repo.php
<?php
namespace App;

class Repo {}

//- /src/ZzCaller.php
<?php
namespace App;

// Named to sort after `Repo`, so the snapshot below still shows the alphabetical order surviving
// *within* a tier -- only the tier itself, project code before vendor, is new.
class ZzCaller
{
    public function run(): void
    {
        new @here@
    }
}
"#,
        &[Query::completion("with nothing typed", "here")],
        expect![[r#"
            with nothing typed
            - Repo [class]  App\Repo
            - ZzCaller [class]  App\ZzCaller
            - AaaWidget [class]  Acme\AaaWidget  + `use Acme\AaaWidget;` at 2:15-2:15
            - accepting the first leaves `new Repo`
        "#]],
    );
}

/// The same prefix one keyword away, where all three of PHP's tables are legal: a bare name is a
/// class, a function or a constant depending on what follows the cursor, and choosing between them
/// from the characters before it would be a guess.
#[test]
fn an_ordinary_expression_still_offers_every_table() {
    check(
        &asking(
            r#"
//- /src/Client.php
<?php
namespace App;

class Client
{
    public function run(): void
    {
        $x = Rep@here@
    }
}
"#,
        ),
        &[Query::completion("in an expression", "here")],
        expect![[r#"
            in an expression
            - REPAID [constant]  App\REPAID
            - repay [function]  App\repay
            - Repeatable [trait]  App\Repeatable
            - Repeated [enum]  App\Repeated
            - Repo [class]  App\Repo
            - Repository [interface]  App\Repository
            - ... and the client must ask again
            - accepting the first leaves `$x = REPAID`
        "#]],
    );
}
