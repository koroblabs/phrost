//! The two `use` keywords that are not imports.
//!
//! PHP writes three different things with one word, and completion answered all three as the first:
//!
//! - `use Acme\Report;` at the top of a file imports a **path**, anchored at the root. That one is
//!   `use_paths` beside this, and it is unchanged.
//! - `use Timestamps;` inside a class body takes a **trait**, and it resolves the way every other
//!   name in the body does. Offered as an import it listed the whole catalog -- interfaces, enums,
//!   functions -- and the qualified path it inserted read as `Ed\Ed\Timestamps` inside `namespace
//!   Ed`, which is a fatal error rather than a bad suggestion.
//! - `function () use ($rows)` captures a **variable**, and nothing else can stand there at all.
//!   That position offered a hundred and ninety-eight class names.
//!
//! Which of the first two a line is takes the tree to say, so the split is made where the view is
//! rather than in the text scanner. The third is textual and exact: the `(` is the mark, and no
//! other `use` in PHP has one.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_use_in_a_class_body_offers_the_traits_and_nothing_else() {
    check(
        r#"
//- /src/Names.php
<?php
namespace Ed;

trait Timestamps {}

interface Publishable {}

enum Status { case Draft; }

class Document {}

//- /src/Report.php
<?php
namespace Ed;

class Report
{
    use @here@
}
"#,
        &[Query::completion("a trait use", "here")],
        expect![[r#"
            a trait use
            - Timestamps [trait]  Ed\Timestamps
            - accepting the first leaves `use Timestamps`
        "#]],
    );
}

#[test]
fn a_closures_capture_list_offers_the_variables_the_scope_binds() {
    check(
        r#"
//- /src/Report.php
<?php
namespace Ed;

class Report
{
    public function run(string $alpha, int $beta): void
    {
        $gamma = 1;
        $write = function () use (@here@) {};
    }
}
"#,
        &[Query::completion("a capture list", "here")],
        expect![[r#"
            a capture list
            - $alpha [variable]
            - $beta [variable]
            - $gamma [variable]
            - $GLOBALS [variable]
            - $_SERVER [variable]
            - $_GET [variable]
            - $_POST [variable]
            - $_FILES [variable]
            - $_COOKIE [variable]
            - $_SESSION [variable]
            - $_REQUEST [variable]
            - $_ENV [variable]
            - accepting the first leaves `$write = function () use ($alpha) {};`
        "#]],
    );
}

/// `$this` is bound by the closure itself and PHP refuses it as a lexical variable outright, and a
/// name the list already holds is a fatal error of its own -- the same reason named-argument
/// completion leaves out a parameter something already filled.
#[test]
fn neither_this_nor_a_name_already_captured_is_offered() {
    check(
        r#"
//- /src/Report.php
<?php
namespace Ed;

class Report
{
    public function run(string $alpha, int $beta): void
    {
        $write = function () use ($alpha, @here@) {};
    }
}
"#,
        &[Query::completion("past the first capture", "here")],
        expect![[r#"
            past the first capture
            - $beta [variable]
            - $GLOBALS [variable]
            - $_SERVER [variable]
            - $_GET [variable]
            - $_POST [variable]
            - $_FILES [variable]
            - $_COOKIE [variable]
            - $_SESSION [variable]
            - $_REQUEST [variable]
            - $_ENV [variable]
            - accepting the first leaves `$write = function () use ($alpha, $beta) {};`
        "#]],
    );
}
