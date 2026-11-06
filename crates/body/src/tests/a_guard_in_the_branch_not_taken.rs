//! A leaving guard narrows the siblings inside its own branch, and nothing in the other one.
//!
//! `visit_preceding_statements` descended into *both* branches of the `if` the offset sits inside,
//! and its inner loop then took every statement that ended before the offset as one that ran. The
//! statements of a branch not taken end before an offset in the next arm of the chain as surely as
//! anything else does, so a leaving guard written inside the first branch put its claim into the
//! second:
//!
//! ```php
//! if (is_array($m)) {
//!     if (!is_int($m)) { throw new RuntimeException('x'); }
//! } elseif (null !== $o) {
//!     $m          // read as an int, on a path where neither line ran
//! }
//! ```
//!
//! Nothing failed while the claim was inert -- narrowing a value to a kind nothing it held was of
//! left it as it was -- and it became a false positive the moment a positive predicate started
//! answering for a value the declaration described some other way. Symfony's `CallbackValidator`
//! is the shape verbatim, three findings on one method.

use expect_test::expect;

use super::check;

#[test]
fn a_leaving_guard_inside_a_branch_says_nothing_in_the_next_arm_of_the_chain() {
    check(
        r#"<?php
class Repo
{
    public function run(string|callable|null $m, mixed $o): void
    {
        if (is_array($m)) {
            if (!is_int($m)) {
                throw new \RuntimeException('x');
            }
        } elseif (null !== $o) {
            $m$0;
        }
    }
}
"#,
        "m",
        expect!["not (array) $m"],
    );
}

/// The claim the walk exists to carry, so the fix is read against what it must not lose: inside the
/// branch, past the leaving guard, the guard is exactly what the value is.
#[test]
fn a_leaving_guard_still_narrows_what_follows_it_in_its_own_branch() {
    check(
        r#"<?php
class Repo
{
    public function run(string|callable|null $m, mixed $o): void
    {
        if (is_array($m)) {
            if (!is_int($m)) {
                throw new \RuntimeException('x');
            }
            $m$0;
        } elseif (null !== $o) {
            $m;
        }
    }
}
"#,
        "m",
        expect!["(int) $m"],
    );
}

/// And after the whole statement, where the branches are behind the position rather than beside it.
/// `<union>` is the harness saying no guard claims anything and the declaration answers.
#[test]
fn a_guard_in_a_branch_says_nothing_after_the_statement_that_held_it() {
    check(
        r#"<?php
class Repo
{
    public function run(string|callable|null $m, mixed $o): void
    {
        if (is_array($m)) {
            if (!is_int($m)) {
                throw new \RuntimeException('x');
            }
        }
        $m$0;
    }
}
"#,
        "m",
        expect!["<union>"],
    );
}
