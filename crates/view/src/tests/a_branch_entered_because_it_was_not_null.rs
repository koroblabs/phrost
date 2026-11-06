//! `if ($s !== null) { $s = implode('|', (array) $s); }` -- around the write, the value was null.
//!
//! A strict null test, `isset` and `!is_null` are one claim in three spellings, and the path that
//! skipped the branch is where it failed. Until it was read, that path held the whole declaration,
//! so a `string|array|null` normalised to a string came out of the `if` still holding the array --
//! hidden for as long as a second write in the same branch put a wrong operand beside it.
//!
//! The other half is the path that cannot exist: a `string $s` guarded the same way is defensive
//! code, and joining a null to it would report every use below.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_path_around_a_strict_null_test_holds_null_and_nothing_else() {
    check_view(
        r#"
//- /stubs/string.php  stub
<?php
function implode(string $separator, array $array): string {}
function trim(string $string, string $characters = " \n\r\t\v\x00"): string {}
//- /src/run.php
<?php
function strict(string|array|null $s): void
{
    if ($s !== null) {
        $s = implode('|', (array) $s);
    }
    $s@strict@;
}

function issetted(string|array|null $s): void
{
    if (isset($s)) {
        $s = implode('|', (array) $s);
    }
    $s@isset@;
}

function called(string|array|null $s): void
{
    if (!is_null($s)) {
        $s = implode('|', (array) $s);
    }
    $s@called@;
}

function defensive(string $s): void
{
    if ($s !== null) {
        $s = trim($s);
    }
    $s@defensive@;
}
"#,
        &[
            Query::variable_type("!== null", "strict"),
            Query::variable_type("isset", "isset"),
            Query::variable_type("!is_null", "called"),
            Query::variable_type("a string nothing can make null", "defensive"),
        ],
        expect![[r#"
            !== null
            - ?string
            isset
            - ?string
            !is_null
            - ?string
            a string nothing can make null
            - string
        "#]],
    );
}
