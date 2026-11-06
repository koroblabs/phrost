//! `sort($term); $term = implode(',', $term);` -- a write after the call is what the name holds.
//!
//! A call written as a statement is read as a guard about what it was passed: a by-reference
//! parameter says what the callee may have left, an `@assert` says what the value must be. Both are
//! claims about the value *at the call*, and a write after it replaces that value -- the rule a
//! guard already follows. The call walk did not ask, so `array` stood over the string written a line
//! later, and every check downstream read the name as the array it no longer was.

use expect_test::expect;

use super::{Query, check_view};

const SORT: &str = r#"
//- /stubs/array.php  stub
<?php
function sort(array &$array, int $flags = 0): true {}
function implode(string $separator, array $array): string {}
"#;

#[test]
fn a_write_after_the_call_replaces_what_the_call_left() {
    check_view(
        &[
            SORT,
            r#"
//- /src/run.php
<?php
function run($term): void
{
    sort($term);
    $term = implode(',', $term);
    $t@after@erm;
}
"#,
        ]
        .concat(),
        &[Query::variable_type("written after the call", "after")],
        expect![[r#"
            written after the call
            - string
        "#]],
    );
}

/// The other order keeps the call's claim: what the call left is the latest thing said.
#[test]
fn a_write_before_the_call_is_what_the_call_overwrites() {
    check_view(
        &[
            SORT,
            r#"
//- /src/run.php
<?php
function run(): void
{
    $term = 'x';
    sort($term);
    $t@after@erm;
}
"#,
        ]
        .concat(),
        &[Query::variable_type("written before the call", "after")],
        expect![[r#"
            written before the call
            - array
        "#]],
    );
}
