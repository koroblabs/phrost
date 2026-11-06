//! `use (&$found)` writes the enclosing scope's variable, and the writes here stop being the answer.
//!
//! `$found = NULL;` beside a closure that captures `&$found` and assigns it says nothing about what
//! `$found` holds afterwards: the write is in the closure's body and it runs whenever whoever was
//! handed the closure decides. Read from the visible assignment alone the name is *definitely null*,
//! which is the strongest claim there is -- 69 reports on one library, on the commonest way a test
//! captures what a callback was given.
//!
//! **The write is what makes the capture interesting.** `$walk = function () use (&$walk) {...}` is
//! how a closure calls itself and only *reads* the name; the assignment beside it is still the whole
//! answer there, and refusing it would take the callable away from every recursive closure in PHP.
//!
//! None of this was visible in the tree at all: lowering dropped the `&`, so `use ($x)` and
//! `use (&$x)` were one shape. It is a [`ExprData::Reference`] now, which is how every other
//! by-reference target is written -- and the two readers of a capture list had to learn to look
//! through it, or the name stops being bound *inside* the closure.

use expect_test::expect;

use super::check;

#[test]
fn a_capture_the_closure_writes_leaves_the_visible_assignment_behind() {
    check(
        r#"<?php
class Repo
{
    public function run(callable $each): void
    {
        $found = NULL;
        $each(function (Row $row) use (&$found): void {
            $found = $row;
        });
        $fo$0und;
    }
}
"#,
        "found",
        expect!["<not declared here>"],
    );
}

/// The same capture, read and not written: `$walk` is how a closure reaches itself, and the
/// assignment beside it is the only thing that says what it is.
#[test]
fn a_capture_the_closure_only_reads_keeps_it() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $walk = function (int $depth) use (&$walk) {
            return $walk($depth - 1);
        };
        $wa$0lk;
    }
}
"#,
        "walk",
        expect!["closure"],
    );
}

/// And the name is still bound *inside* the closure, which is what the `&` wrapper put at risk: a
/// reader of the capture list that stops at the wrapper sees no captured name at all, and every
/// write inside reads as a local nothing else uses.
#[test]
fn the_captured_name_is_still_in_scope_inside_the_closure() {
    super::check_names(
        r#"<?php
class Repo
{
    public function run(callable $each): void
    {
        $found = NULL;
        $each(function (Row $row) use (&$found): void {
            $fo$0und;
        });
    }
}
"#,
        expect![[r#"
            $row
            $found"#]],
    );
}
