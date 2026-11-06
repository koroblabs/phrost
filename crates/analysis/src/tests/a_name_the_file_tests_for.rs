//! `function_exists('ssh2_connect')` and its neighbours, which say a name may be absent.
//!
//! PHP has no other way to write "this extension may not be installed", and the branch behind the
//! test is correct whether or not it is. Reporting the name as undefined tells the author what
//! they already knew and handled -- 683 findings across four corpora, every one inside the guard
//! that rules it out.
//!
//! The test is read per *file* rather than per branch, and the reason is where real code puts it:
//! a PECL class is named in the type hints of a whole trait and constructed once behind a
//! `class_exists`, and WordPress tests for one function of an extension and calls seven of its
//! neighbours elsewhere in the file. What that gives up is a file that tests a name in one place
//! and calls it unguarded in another, which is a trade this check can afford -- what it is for is
//! catching a typo, and a typo is never the name somebody wrote a `function_exists` for.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_function_the_file_tests_for_is_not_reported() {
    check(
        r#"
//- /src/app.php
<?php

function function_exists(string $name): bool {}
function extension_loaded(string $name): bool {}
function class_exists(string $name): bool {}

function connect(string $host): void
{
    if (!function_exists('ssh2_connect')) {
        return;
    }

    ssh2_connect($host, 22);
}
"#,
        expect![""],
    );
}

/// An extension names no function, so what connects the two is the convention every extension
/// follows: its functions carry its own name and an underscore.
#[test]
fn an_extension_the_file_tests_for_covers_its_own_prefix() {
    check(
        r#"
//- /src/app.php
<?php

function function_exists(string $name): bool {}
function extension_loaded(string $name): bool {}
function class_exists(string $name): bool {}

function disconnect(): void
{
    if (extension_loaded('ssh2')) {
        ssh2_disconnect(null);
    }
}
"#,
        expect![""],
    );
}

/// A class test is written with `::class` as often as with a string, and both name the class.
#[test]
fn a_class_the_file_tests_for_is_not_reported() {
    check(
        r#"
//- /src/app.php
<?php

function function_exists(string $name): bool {}
function extension_loaded(string $name): bool {}
function class_exists(string $name): bool {}

function build(): void
{
    if (class_exists(\Relay\Relay::class)) {
        $relay = new \Relay\Relay();
        $relay->connect();
    }
}
"#,
        expect![""],
    );
}

/// The control for the three above: a name nothing tests for still reports, so an empty snapshot
/// there is the test being read rather than the check being off.
#[test]
fn a_name_nothing_tests_for_still_reports() {
    check(
        r#"
//- /src/app.php
<?php

function function_exists(string $name): bool {}
function extension_loaded(string $name): bool {}
function class_exists(string $name): bool {}

function connect(string $host): void
{
    ssh2_connect($host, 22);
}
"#,
        expect![[r#"
            9:5-9:17 error [undefined-function] Undefined function ssh2_connect()
        "#]],
    );
}
