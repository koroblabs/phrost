//! A construct whose every arm writes leaves no path that skipped them.
//!
//! The join a conditional write makes has three operands -- the writes, what the name held before
//! the construct, and what the guard that skipped them proved -- and the last two are about a path
//! that sometimes does not exist. `if ($c) { $x = a(); } else { $x = b(); }` writes on both, so the
//! value after it is one of the two and never what it was before.
//!
//! Only where the arms are *all* of them. An `if` with no `else`, or an arm whose write is nested a
//! level deeper, leaves a path that wrote nothing -- and the third operand is then the answer for
//! it.

use expect_test::expect;

use super::check;

#[test]
fn an_if_and_its_else_both_writing_leave_no_earlier_value_among_the_operands() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $v, bool $c): void
    {
        if ($c) {
            $v = 'left';
        } else {
            $v = 'right';
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["(string) or (string)"],
    );
}

/// The same shape with the `else` missing, which is the one the third operand exists for.
#[test]
fn an_if_with_no_else_keeps_what_the_name_held_before_it() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $v, bool $c): void
    {
        if ($c) {
            $v = 'left';
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["(string) or $v before"],
    );
}

/// And with the second write a level deeper, where the arm as a whole writes nothing sure.
#[test]
fn a_write_nested_inside_an_arm_does_not_fill_it() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $v, bool $c, bool $d): void
    {
        if ($c) {
            $v = 'left';
        } else {
            if ($d) {
                $v = 'right';
            }
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["(string) or (string) or $v before"],
    );
}

/// A construct that is the whole of an arm is followed: every path through the outer arm writes, so
/// no path skips them all. This used to be refused on a measurement rather than a principle --
/// following it needed `exclusive_arms`, then a scan of every branch node in the file, once per
/// nested construct per lookup, and that took `check` on one project from 180 ms to 350-520. The arms
/// are indexed by their start now, which is what the refusal said it was waiting for.
#[test]
fn a_whole_construct_inside_an_arm_is_followed() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $v, bool $c, bool $d): void
    {
        if ($c) {
            if ($d) {
                $v = 'left';
            } else {
                $v = 'right';
            }
        } else {
            $v = 'other';
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["(string) or (string) or (string)"],
    );
}
