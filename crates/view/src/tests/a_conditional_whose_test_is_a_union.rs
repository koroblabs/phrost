//! `($array is non-empty-array|non-empty-list ? V : null)`, which is how the 8.5 stubs write
//! `array_first` and `array_last`.
//!
//! The test reader took a single type, so a union of two tests was refused whole and both functions
//! answered whatever the declaration said -- `mixed`. Every operand has to ask the same question,
//! because the vocabulary carries one test and a union asking two different things is two.
//!
//! What fell out of reading it is the more interesting half. When nothing settles a non-empty test,
//! the answer used to be the *else* branch, on the ground that lowering records such a test only
//! where absence is the whole difference between the branches -- so the else admits both. That
//! holds for `reset`, whose else is `V|false`, and is wrong for `array_first`, whose else is a bare
//! `null`: answering it said the array is empty on every call with no guard above it. Both branches
//! together are what the declaration says, and that is what an unsettled test answers with now.

use expect_test::expect;

use super::{Query, check_view};

const STUB: &str = r#"
//- /vendor/php/stubs.php
<?php

/**
 * @template V
 *
 * @param array<array-key, V> $array
 *
 * @return ($array is non-empty-array|non-empty-list ? V : null)
 */
function array_first(array $array): mixed {}
"#;

#[test]
fn a_guard_settles_the_union_of_tests() {
    check_view(
        &format!(
            "{STUB}{}",
            r#"
//- /src/app.php
<?php
namespace App;

class Row {}

class Host
{
    /** @param list<Row> $rows */
    public function guarded(array $rows): void
    {
        if (empty($rows)) {
            return;
        }

        $row = array_first($rows);
        $r@guarded@ow;
    }

    /** @param list<Row> $rows */
    public function unguarded(array $rows): void
    {
        $row = array_first($rows);
        $r@unguarded@ow;
    }
}
"#
        ),
        &[
            Query::variable_type("the array was proven to hold something", "guarded"),
            Query::variable_type("nothing ruled the absence out", "unguarded"),
        ],
        expect![[r#"
            the array was proven to hold something
            - Row
            nothing ruled the absence out
            - ?Row
        "#]],
    );
}
