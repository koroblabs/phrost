//! `match (true)`'s default arm after two arms that each ruled something out.
//!
//! Every earlier arm failed before a later arm's conditions were evaluated, not only before its
//! body ran, and a guard's proof reads the value where its condition is written. Covering only the
//! bodies, the default arm below was proved from `is_array` failing -- read at the second arm's
//! condition, where the first arm's `is_null` was not in effect -- and the null came back: Laravel's
//! own `context()` helper was reported for passing a value the arm above it had just ruled out.

use expect_test::expect;

use crate::tests::{Query, check};

const FIXTURE: &str = r#"
//- /src/A.php
<?php
namespace App;

/** @param array|string|null $key */
function context($key = null): int
{
    return match (true) {
        is_null($key) => 0,
        is_array($key) => 1,
        default => strlen($k@default@ey),
    };
}

/** @param array|string|null $key */
function second($key = null): int
{
    return match (true) {
        is_null($key) => 0,
        is_array($k@second@ey) => 1,
        default => 2,
    };
}
"#;

#[test]
fn a_later_arm_sees_every_earlier_arm_fail() {
    check(
        FIXTURE,
        &[
            Query::hover("the default arm, past both", "default"),
            Query::hover("the second arm's own condition, past the first", "second"),
        ],
        expect![[r#"
            the default arm, past both
            - code: string $key
            - note: narrowed by a guard
            the second arm's own condition, past the first
            - code: array|string $key
            - note: narrowed by a guard
        "#]],
    );
}
