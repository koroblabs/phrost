//! `if (...) { foreach ($terms as $term) {...} } else { $term; }` -- one `if` takes one arm.
//!
//! An assignment in the arm the position is not in has never reached it, and the reach rule has
//! always said so. A loop's binding was ranked by position alone, so the `else` read the loop's
//! element -- of a list split out of the very `$term` it then replaced. The binding answers to the
//! rule every write does.

use expect_test::expect;

use super::check;

#[test]
fn a_loop_in_the_then_arm_binds_nothing_in_the_else() {
    check(
        r#"<?php
function f(string $term, bool $plus): void
{
    $term = trim($term);
    if ($plus) {
        foreach (explode('+', $term) as $term) {
            echo $term;
        }
    } else {
        $$0term;
    }
}
"#,
        "term",
        expect!["trim()"],
    );
}

/// After the `if` the loop may have run, and position is still the rule there.
#[test]
fn after_the_if_the_loop_is_still_the_latest_binding() {
    check(
        r#"<?php
function f(string $term, bool $plus): void
{
    $term = trim($term);
    if ($plus) {
        foreach (explode('+', $term) as $term) {
            echo $term;
        }
    }
    $$0term;
}
"#,
        "term",
        expect!["explode()[element]"],
    );
}
