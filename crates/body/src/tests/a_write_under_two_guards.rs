//! The way around a write is every way around it, and a write under two guards has two.
//!
//! `if ($a) { if ($x === null) { $x = 'd'; } }` is skipped where `!$a`, which tests nothing about
//! `$x`, as well as where `$x` was not null -- and read by the guard the write sits under it said the
//! value could not be null past the `if`. What holds on every way is what the join may claim: one
//! way saying nothing ends it, and two saying different things keep only what both imply.

use expect_test::expect;

use super::check;

#[test]
fn a_way_around_that_tests_nothing_proves_nothing() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $x, bool $a): void
    {
        if ($a) {
            if ($x === null) {
                $x = 'd';
            }
        }
        $$0x;
    }
}
"#,
        "x",
        expect!["(string) or $x before"],
    );
}

/// The other way is gone when its arm leaves, and the one left is the inner guard's.
#[test]
fn a_way_around_that_leaves_is_not_one() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $x, bool $a): void
    {
        if ($a) {
            if ($x === null) {
                $x = 'd';
            }
        } else {
            return;
        }
        $$0x;
    }
}
"#,
        "x",
        expect!["(string) or $x before minus null"],
    );
}

/// Symfony's `FixedWindowLimiter`: one way around holds a `CalendarAlignedWindow`, the other a
/// `Window`, and what both say is that there is a window.
#[test]
fn two_ways_proving_different_classes_both_prove_the_value_is_there() {
    check(
        r#"<?php
class Window {}
class CalendarAlignedWindow extends Window {}
class Limiter
{
    public function run(?Window $w, bool $anchor, bool $stale): void
    {
        if ($anchor) {
            if (!$w instanceof CalendarAlignedWindow || $stale) {
                $w = new CalendarAlignedWindow();
            }
        } elseif (!$w instanceof Window) {
            $w = new Window();
        }
        $$0w;
    }
}
"#,
        "w",
        expect!["new CalendarAlignedWindow or new Window or $w before minus null"],
    );
}
