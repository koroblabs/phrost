//! `if ($b) { $c = explode(',', $c); $c = $c[1]; }` -- two writes in one branch are a sequence.
//!
//! Each is only *possible* at a read past the `if`, which is what used to hand both to the join: the
//! answer was the list and the element at once, and every string parameter the name then reached
//! reported it. What the second write did to the first is the tree's to say, so it is said here;
//! what the join then makes of the survivors is the view's.
//!
//! The refusals are the half that matters. A write replaces another only when nothing can carry the
//! earlier value past it, and three things can: a jump between the two, a handler the read is in,
//! and a later write nested deeper than the earlier one, which the earlier one's branch can skip.

use expect_test::expect;

use super::check;

#[test]
fn the_last_write_in_a_branch_is_its_only_operand() {
    check(
        r#"<?php
function f(string $c, bool $b): void
{
    if ($b) {
        $c = explode(',', $c);
        $c = trim($c[1]);
    }
    $$0c;
}
"#,
        "c",
        expect!["trim() or $c before"],
    );
}

/// The earlier write may be nested deeper -- leaving its branch arrives at the later one.
#[test]
fn a_write_after_the_branch_holding_the_earlier_one_replaces_it() {
    check(
        r#"<?php
function f(string $c, bool $b, bool $d): void
{
    if ($b) {
        if ($d) {
            $c = [1];
        }
        $c = 'x';
    }
    $$0c;
}
"#,
        "c",
        expect!["(string) or $c before"],
    );
}

/// The other way round it does not: the inner branch can be skipped, and then the earlier value is
/// what arrives.
#[test]
fn a_later_write_nested_deeper_replaces_nothing() {
    check(
        r#"<?php
function f(string $c, bool $b, bool $d): void
{
    if ($b) {
        $c = [1];
        if ($d) {
            $c = 'x';
        }
    }
    $$0c;
}
"#,
        "c",
        expect!["list literal of 1 or (string) or $c before"],
    );
}

/// A `break` between the two leaves the loop with the earlier value.
#[test]
fn a_jump_between_the_two_carries_the_earlier_value_out() {
    check(
        r#"<?php
function f(array $rows): void
{
    $c = '';
    foreach ($rows as $row) {
        if ($row) {
            $c = [1];
            if ($row > 1) {
                break;
            }
            $c = 'x';
        }
    }
    $$0c;
}
"#,
        "c",
        expect!["list literal of 1 or (string) or $c before"],
    );
}

/// A `catch` the read is in is reached from between the two, by whatever the call throws.
#[test]
fn a_handler_the_read_is_in_sees_the_earlier_value() {
    check(
        r#"<?php
function f(string $c, bool $b): void
{
    try {
        if ($b) {
            $c = [1];
            g();
            $c = 'x';
        }
    } catch (Exception $e) {
        $$0c;
    }
}
"#,
        "c",
        expect!["list literal of 1 or (string) or $c before"],
    );
}
