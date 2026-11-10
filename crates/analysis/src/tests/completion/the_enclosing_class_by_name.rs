//! `Archive::helper()` inside `Archive`: the enclosing class's own name before `::`.
//!
//! Through a class's name an instance method is a static call PHP 8 refuses -- unless the code
//! runs as that class or as one descending from it, where the name is `self` or `parent` spelled
//! out and the call is made on `$this`. `PclZip::privErrorLog(...)` is written sixty times in the
//! class WordPress ships and goto answered every one of them, while completion offered the
//! constants and stopped. One file to a marker: an unfinished statement each.

use expect_test::expect;

use crate::tests::{Query, check};

const ARCHIVE: &str = r#"
//- /src/Archive.php
<?php
namespace App;

class Archive
{
    public const LIMIT = 10;

    public function logError(int $code): int {}

    public static function open(string $path): self {}
}
"#;

fn check_beside_archive(client: &str, label: &'static str, expect: expect_test::Expect) {
    check(
        &format!("{ARCHIVE}\n{client}"),
        &[Query::completion(label, "here")],
        expect,
    );
}

/// In a subclass the ancestor's name is `parent` written out: its instance method is offered
/// beside the static one and the constant.
#[test]
fn a_subclass_reaches_an_instance_method_through_its_ancestors_name() {
    check_beside_archive(
        r#"
//- /src/Zip.php
<?php
namespace App;

class Zip extends Archive
{
    public function add(): int
    {
        return Archive::lo@here@
    }
}
"#,
        "an ancestor by name",
        expect![[r#"
            an ancestor by name
            - logError [method]  : int  writes `(|)`
            - ... and the client must ask again
            - accepting the first leaves `return Archive::logError`
        "#]],
    );
}

/// The shape the report was: the class naming itself, in its own method.
#[test]
fn a_class_reaches_its_own_instance_method_through_its_own_name() {
    check(
        r#"
//- /src/Archive.php
<?php
namespace App;

class Archive
{
    public function logError(int $code): int {}

    public function add(): int
    {
        return Archive::lo@here@
    }
}
"#,
        &[Query::completion("the class by its own name", "here")],
        expect![[r#"
            the class by its own name
            - logError [method]  : int  writes `(|)`
            - ... and the client must ask again
            - accepting the first leaves `return Archive::logError`
        "#]],
    );
}

/// Anywhere else the same line is a static call to a method that is not static, and it stays
/// refused: only what `::` can reach on a class nobody here is.
#[test]
fn an_unrelated_class_does_not() {
    check_beside_archive(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(): int
    {
        return Archive::lo@here@
    }
}
"#,
        "a class nobody here is",
        expect![[r#"
            a class nobody here is
            - (nothing offered)
        "#]],
    );
}
