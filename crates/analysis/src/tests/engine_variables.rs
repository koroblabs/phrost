//! The types the runtime gives the variables no file declares.
//!
//! `$_SERVER` and its siblings are bound before a line of the project runs, so every read of one
//! was untyped: an index that has no answer for the commonest values in a PHP request. The
//! declaration is the engine's and lives in `phrost_index::superglobals`.

use expect_test::expect;

use crate::tests::{Query, check};

/// A key of `$_SERVER` is whatever that key holds, which is not always a string.
///
/// The two most-read keys on one real checkout are `REQUEST_TIME` and `REQUEST_TIME_FLOAT`, and
/// they are the two that are not -- which is why this is a shape and not `array<string, string>`.
#[test]
fn a_server_key_is_the_type_that_key_holds() {
    check(
        r#"
//- /src/app.php
<?php

function run(): void
{
    $when = $_SERVER['REQUEST_TIME'];
    $wh@when@en;
}
"#,
        &[Query::hover("the request time", "when")],
        expect![[r#"
            the request time
            - code: int $when
            - note: assigned
        "#]],
    );
}

#[test]
fn a_string_key_of_the_same_shape_is_a_string() {
    check(
        r#"
//- /src/app.php
<?php

function run(): void
{
    $uri = $_SERVER['REQUEST_URI'];
    $u@uri@ri;
}
"#,
        &[Query::hover("the request uri", "uri")],
        expect![[r#"
            the request uri
            - code: string $uri
            - note: assigned
        "#]],
    );
}

/// A key the shape does not name says nothing, rather than saying `string`.
///
/// The safe half of choosing a shape: a header nobody documented is still a real key, and an
/// answer of `string` there would be a guess this project's own rule refuses.
#[test]
fn a_key_the_shape_does_not_name_answers_nothing() {
    check(
        r#"
//- /src/app.php
<?php

function run(): void
{
    $vendor = $_SERVER['HTTP_X_VENDOR_TOKEN'];
    $ven@vendor@dor;
}
"#,
        &[Query::hover("an undocumented header", "vendor")],
        expect![[r#"
            an undocumented header
            - code: $vendor
            - note: assigned
            - note: no declared type
        "#]],
    );
}

/// What a request submits is a string or an array, because `?a[]=1` makes it the second.
#[test]
fn what_a_request_submits_is_either_a_string_or_an_array() {
    check(
        r#"
//- /src/app.php
<?php

function run(): void
{
    $id = $_GET['id'];
    $i@id@d;
}
"#,
        &[Query::hover("a query parameter", "id")],
        expect![[r#"
            a query parameter
            - code: string|array $id
            - note: assigned
        "#]],
    );
}

/// A file that writes the name itself wins: the engine's answer is asked last, where the body has
/// no binding to give.
#[test]
fn a_file_that_assigns_a_superglobal_says_what_it_holds() {
    check(
        r#"
//- /src/app.php
<?php

function run(int $count): void
{
    $_SESSION = $count;
    $s@session@ession = $_SESSION;
}
"#,
        &[Query::hover("assigned over", "session")],
        expect![[r#"
            assigned over
            - code: int $session
            - note: assigned
        "#]],
    );
}
