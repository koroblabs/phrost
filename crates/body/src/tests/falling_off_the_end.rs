//! Whether control can run past the last statement of a body.
//!
//! The opposite question to `leaving.rs`, and the opposite conservatism: that one may only answer
//! "yes, certainly", and this one may only answer "no, certainly". Every shape here is one where a
//! wrong answer costs a report on code that works -- a `try` whose halves both return, a loop with
//! no exit, a `switch` that covers everything.
//!
//! One snapshot per fixture listing every function it declares, so a fixture buys a verdict on each
//! shape at once and a rule that changes one silently is visible as a changed line.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner};

/// Renders `name: falls off the end` or `name: leaves` for every free function in `source`.
fn check(source: &str, expect: Expect) {
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        source,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );

    let hir = &lowered.hir;
    let rendered = hir
        .functions()
        .map(|(_, function)| {
            let body = function.body.expect("bodies were lowered");
            // Nothing is known to return `never` here: these are the shapes syntax settles on its
            // own, and the resolution half is measured where a project exists.
            let falls = crate::falls_off_end(hir, hir.body(body).statements, &|_, _| false);
            format!(
                "{}: {}\n",
                function.name,
                match falls {
                    true => "falls off the end",
                    false => "leaves",
                }
            )
        })
        .collect::<String>();

    expect.assert_eq(&rendered);
}

#[test]
fn an_if_with_no_else_falls_through_and_one_with_a_returning_else_does_not() {
    check(
        r#"<?php
function guarded(int $n): string
{
    if ($n > 0) {
        return 'positive';
    }
}

function both(int $n): string
{
    if ($n > 0) {
        return 'positive';
    } else {
        return 'other';
    }
}

function after(int $n): string
{
    if ($n > 0) {
        return 'positive';
    }
    return 'other';
}
"#,
        expect![[r#"
            guarded: falls off the end
            both: leaves
            after: leaves
        "#]],
    );
}

#[test]
fn an_empty_body_falls_off_its_own_first_statement() {
    check(
        r#"<?php
function blank(): string
{
}
"#,
        expect![[r#"
            blank: falls off the end
        "#]],
    );
}

#[test]
fn throw_and_exit_end_the_flow_and_a_bare_call_does_not() {
    check(
        r#"<?php
function thrown(): string
{
    throw new \RuntimeException('no');
}

function stopped(): string
{
    exit(1);
}

function called(): string
{
    \trigger_error('no');
}
"#,
        expect![[r#"
            thrown: leaves
            stopped: leaves
            called: falls off the end
        "#]],
    );
}

#[test]
fn a_try_leaves_when_its_body_and_every_handler_do() {
    check(
        r#"<?php
function handled(): string
{
    try {
        return 'a';
    } catch (\Throwable $e) {
        return 'b';
    }
}

function unhandled(): string
{
    try {
        return 'a';
    } catch (\Throwable $e) {
    }
}

function finally_decides(): string
{
    try {
        \work();
    } finally {
        return 'always';
    }
}

function finally_cleans_up(): string
{
    try {
        return 'a';
    } finally {
        \cleanup();
    }
}
"#,
        expect![[r#"
            handled: leaves
            unhandled: falls off the end
            finally_decides: leaves
            finally_cleans_up: leaves
        "#]],
    );
}

#[test]
fn a_loop_with_no_exit_leaves_and_one_that_can_break_out_does_not() {
    check(
        r#"<?php
function endless(): string
{
    while (true) {
        if (\rand() > 3) {
            return 'x';
        }
    }
}

function endless_with_a_break(): string
{
    while (true) {
        if (\rand() > 3) {
            break;
        }
    }
}

function endless_for(): string
{
    for (;;) {
        \work();
    }
}

function conditional(): string
{
    while (\rand() > 3) {
        return 'x';
    }
}

function over_a_list(array $rows): string
{
    foreach ($rows as $row) {
        return 'x';
    }
}
"#,
        expect![[r#"
            endless: leaves
            endless_with_a_break: falls off the end
            endless_for: leaves
            conditional: falls off the end
            over_a_list: falls off the end
        "#]],
    );
}

#[test]
fn a_switch_leaves_only_with_a_default_and_no_way_out_of_it() {
    check(
        r#"<?php
function covered(int $n): string
{
    switch ($n) {
        case 1:
            return 'one';
        default:
            return 'other';
    }
}

function no_default(int $n): string
{
    switch ($n) {
        case 1:
            return 'one';
        case 2:
            return 'two';
    }
}

function breaks_out(int $n): string
{
    switch ($n) {
        case 1:
            return 'one';
        default:
            break;
    }
}

function falls_out_of_the_last_arm(int $n): string
{
    switch ($n) {
        case 1:
            return 'one';
        default:
            \work();
    }
}

function falls_into_the_default(int $n): string
{
    switch ($n) {
        case 1:
            \work();
        default:
            return 'other';
    }
}
"#,
        expect![[r#"
            covered: leaves
            no_default: falls off the end
            breaks_out: falls off the end
            falls_out_of_the_last_arm: falls off the end
            falls_into_the_default: leaves
        "#]],
    );
}

#[test]
fn a_match_written_as_a_statement_leaves_when_every_arm_does() {
    check(
        r#"<?php
function every_arm_throws(int $n): string
{
    match ($n) {
        1 => throw new \RuntimeException('one'),
        default => throw new \RuntimeException('other'),
    };
}

function no_default_arm(int $n): string
{
    match ($n) {
        1 => throw new \RuntimeException('one'),
    };
}

function one_arm_produces_a_value(int $n): string
{
    match ($n) {
        1 => \work(),
        default => throw new \RuntimeException('other'),
    };
}
"#,
        expect![[r#"
            every_arm_throws: leaves
            no_default_arm: falls off the end
            one_arm_produces_a_value: falls off the end
        "#]],
    );
}

#[test]
fn a_break_out_of_a_loop_is_not_a_way_out_of_the_function() {
    check(
        r#"<?php
function loops(array $rows): string
{
    foreach ($rows as $row) {
        if ($row) {
            break;
        }
    }
}
"#,
        expect![[r#"
            loops: falls off the end
        "#]],
    );
}

/// Found on Symfony's `PhpArrayAdapter::get`, which ends its one `catch` in `goto get_from_pool`
/// and read as a method that returns nothing. A `goto` and an `echo` are the same node here, so a
/// block ending in either is a block whose fate this declines to state.
#[test]
fn a_block_ending_in_a_statement_this_cannot_read_is_not_called_a_fall_through() {
    check(
        r#"<?php
function jumps(int $n): string
{
    retry:
    try {
        return \work($n);
    } catch (\Throwable $e) {
        goto retry;
    }
}

function prints(int $n): string
{
    if ($n > 0) {
        return 'positive';
    }
    echo $n;
}

function unsets(array $rows): string
{
    unset($rows[0]);
}
"#,
        expect![[r#"
            jumps: leaves
            prints: falls off the end
            unsets: falls off the end
        "#]],
    );
}

#[test]
fn a_statement_after_one_that_leaves_does_not_bring_the_flow_back() {
    check(
        r#"<?php
function dead_tail(int $n): string
{
    return 'ok';
    $n = 1;
}
"#,
        expect![[r#"
            dead_tail: leaves
        "#]],
    );
}
