//! `if ($row === [null]) { return; }`: past it, the record is not that array -- and where every slot
//! but one already holds the literal's value, the one left is not the literal's.
//!
//! PHP's `===` on arrays compares every key and value, so failing it says some slot differs; with the
//! others pinned, that slot is the one. A slot no other is pinned beside rules nothing out, and nor
//! does a literal string or number, which names one value of a type with many.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_failed_identity_takes_the_literal_off_the_one_slot_that_can_differ() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

function one(?string $value): void
{
    $row = [$value];
    if ($row === [null]) {
        return;
    }
    $o@one@ne = $row;
}

function pinned(?string $value): void
{
    $row = [$value, null];
    if ($row !== [null, null]) {
        $p@pinned@inned = $row;
    }
}

function falseSlot(string|false $value): void
{
    $row = ['found' => $value];
    if ($row === ['found' => false]) {
        return;
    }
    $f@false@alse = $row;
}

function two(?string $first, ?string $second): void
{
    $row = [$first, $second];
    if ($row === [null, null]) {
        return;
    }
    $t@two@wo = $row;
}

function aString(?string $value): void
{
    $row = [$value];
    if ($row === ['none']) {
        return;
    }
    $s@string@tring = $row;
}

function otherKeys(?string $value): void
{
    $row = ['id' => $value];
    if ($row === [null]) {
        return;
    }
    $k@keys@eys = $row;
}

function held(?string $value): void
{
    $row = [$value];
    if ($row === [null]) {
        $h@held@eld = $row;
    }
}
"#,
        &[
            Query::variable_type("one slot, compared with null", "one"),
            Query::variable_type("the other slot already null", "pinned"),
            Query::variable_type("a keyed slot, compared with false", "false"),
            Query::variable_type("two slots either of which may differ", "two"),
            Query::variable_type("a string literal names one value of many", "string"),
            Query::variable_type("keys the literal does not have", "keys"),
            Query::variable_type("the side where the identity held", "held"),
        ],
        expect![[r#"
            one slot, compared with null
            - array{0: string}
            the other slot already null
            - array{0: string, 1: null}
            a keyed slot, compared with false
            - array{found: string}
            two slots either of which may differ
            - array{0: ?string, 1: ?string}
            a string literal names one value of many
            - array{0: ?string}
            keys the literal does not have
            - array{id: ?string}
            the side where the identity held
            - array{0: ?string}
        "#]],
    );
}
