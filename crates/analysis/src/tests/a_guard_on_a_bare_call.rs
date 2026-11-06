//! `if (wp_get_referer()) { wp_safe_redirect(wp_get_referer()); }`.
//!
//! The guard a method call gets for free from its receiver, written by a project that has no
//! classes. Two calls to one function are one slot, and they were two: the root carried the
//! `NameRefId` the call was written as, which is per occurrence, so nothing compared equal and
//! nothing narrowed. `PathStep::MethodReturn` states that rule for a method and this half had never
//! learned it.
//!
//! Only where every argument is written out, which is what keeps the slot from merging two
//! different calls: `current($rows)` and `current($totals)` carry the same empty list of literals
//! and would otherwise be one value -- and `current(current($this->iterations))` would narrow the
//! outer call to what a guard proved about the inner.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_guard_on_a_call_narrows_the_same_call_written_again() {
    check(
        r#"
//- /app.php
<?php
function referer(): string|false
{
    return 'x';
}

function takes(string $where): void {}

function go(): void
{
    if (referer()) {
        takes(referer());
    }
    takes(referer());
}
"#,
        expect![[r#"
            14:11-14:20 warning [possibly-false-argument] $where takes string, and this may be false
        "#]],
    );
}

#[test]
fn a_call_whose_arguments_are_not_all_written_out_is_not_a_slot() {
    // The over-merge this refuses: two calls to one function with two different arguments carry the
    // same (empty) list of literals, so a guard about one would narrow the other.
    check(
        r#"
//- /app.php
<?php
function first(array $rows): string|false
{
    return 'x';
}

function takes(string $name): void {}

function go(array $left, array $right): void
{
    if (first($left)) {
        takes(first($right));
    }
}
"#,
        expect![[r#"
            12:15-12:28 warning [possibly-false-argument] $name takes string, and this may be false
        "#]],
    );
}

#[test]
fn a_literal_argument_is_written_out_and_keeps_the_slot() {
    check(
        r#"
//- /app.php
<?php
function option(string $key): string|false
{
    return 'x';
}

function takes(string $name): void {}

function go(): void
{
    if (option('home')) {
        takes(option('home'));
    }
    takes(option('home'));
}
"#,
        // The call outside the guard is the control: the same expression, reported, which is what
        // makes the silence inside the guard an answer rather than an absence.
        expect![[r#"
            14:11-14:25 warning [possibly-false-argument] $name takes string, and this may be false
        "#]],
    );
}
