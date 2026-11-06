//! `if (null === $n) { $n = 1; } elseif ($n > 9) { $n = 9; }` -- around both writes, not null.
//!
//! The `elseif` test says nothing about `$n` being null, so what its write's skipped path holds is
//! read off the outer test -- and that complement is the first arm, which wrote. Taken as the path
//! around both, it was "null"; the first write's complement, the `else` the `elseif` can be skipped
//! in, is the claim that describes it. A claim stands aside only where the arm it describes is
//! written on every way through.

use expect_test::expect;

use super::check;

#[test]
fn an_arm_written_throughout_is_no_path_around_the_writes() {
    check(
        r#"<?php
function f(?int $n): void
{
    if (null === $n) {
        $n = 1;
    } elseif ($n > 9) {
        $n = 9;
    }
    $$0n;
}
"#,
        "n",
        expect!["(int) or (int) or $n before minus null"],
    );
}

/// The first arm skippable inside itself keeps its complement honest about part of the path, and
/// the two claims then disagree -- so nothing is claimed, rather than either one.
#[test]
fn an_arm_written_only_sometimes_leaves_both_claims_partial() {
    check(
        r#"<?php
function f(?int $n, bool $c): void
{
    if (null === $n) {
        if ($c) {
            $n = 1;
        }
    } elseif ($n > 9) {
        $n = 9;
    }
    $$0n;
}
"#,
        "n",
        expect!["(int) or (int) or $n before"],
    );
}
