//! `$row === [null]` failing: the value is not that array.
//!
//! The claim carries the literal rather than a type, because what not being it rules out of each
//! slot depends on what the value's other slots hold. Strict only -- `[0] == [null]` holds -- and
//! only the side that failed; the side that held is the non-null claim every identity against an
//! array already makes.

use expect_test::expect;

use super::check;

#[test]
fn an_identity_that_failed_names_the_literal() {
    check(
        r#"<?php
function run(?string $value): void
{
    $row = [$value];
    if ($row === [null]) {
        return;
    }
    $r$0ow;
}
"#,
        "row",
        expect!["$row not identical to [null]"],
    );
}

#[test]
fn a_keyed_literal_keeps_its_keys() {
    check(
        r#"<?php
function run(array $row): void
{
    if ($row !== ['id' => null, 'ok' => false]) {
        $r$0ow;
    }
}
"#,
        "row",
        expect!["$row not identical to [id => null, ok => false]"],
    );
}

#[test]
fn a_literal_holding_a_value_proves_nothing() {
    // `$other` is a value this layer cannot compare slot by slot.
    check(
        r#"<?php
function run(array $row, $other): void
{
    if ($row === [$other]) {
        return;
    }
    $r$0ow;
}
"#,
        "row",
        expect!["array"],
    );
}

#[test]
fn a_loose_comparison_proves_nothing() {
    check(
        r#"<?php
function run(array $row): void
{
    if ($row == [null]) {
        return;
    }
    $r$0ow;
}
"#,
        "row",
        expect!["array"],
    );
}
