//! The statements nothing reaches, and the ones that only look like it.
//!
//! Two families of silence decide whether this is liveable. A `return` inside an `if` says nothing
//! about the block around it, and a `switch` arm's `break` says nothing about the arm after it --
//! get either wrong and every guarded function in a project is reported. The third is the one PHP
//! itself creates: a declaration written after a `return` is hoisted and still exists, so a dead run
//! stops at the first statement this tree cannot tell apart from one.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner};

/// Renders every unreachable run in `source` as `line:col-line:col` with the text it covers.
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
    let lines = phrost_model::LineIndex::new(source);
    let mut bodies = Vec::new();
    bodies.extend(hir.root_body());
    for (_, function) in hir.functions() {
        bodies.extend(function.body);
    }
    for (_, class) in hir.classes() {
        for &id in hir.member_list(class.members) {
            bodies.extend(hir.member(id).body);
        }
    }

    let rendered = bodies
        .into_iter()
        .flat_map(|body| crate::unreachable_regions(hir, hir.body(body).statements))
        .map(|range| {
            let start = lines.line_col(range.start());
            let end = lines.line_col(range.end());
            format!(
                "{}:{}-{}:{} {}\n",
                start.line + 1,
                start.col + 1,
                end.line + 1,
                end.col + 1,
                source[usize::from(range.start())..usize::from(range.end())]
                    .split('\n')
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .collect::<String>();

    expect.assert_eq(&match rendered.is_empty() {
        true => "<everything is reachable>".to_owned(),
        false => rendered,
    });
}

#[test]
fn statements_after_a_return_are_one_run() {
    check(
        r#"<?php
function after_a_return(int $n): string
{
    return 'ok';
    $n = 1;
    $n = 2;
}
"#,
        expect![[r#"
            5:5-6:12 $n = 1; $n = 2;
        "#]],
    );
}

#[test]
fn a_return_inside_an_if_leaves_the_block_around_it_alone() {
    check(
        r#"<?php
function guarded(int $n): string
{
    if ($n > 0) {
        return 'positive';
    }
    $n = 1;
    return 'other';
}
"#,
        expect![["<everything is reachable>"]],
    );
}

#[test]
fn a_switch_arm_that_breaks_says_nothing_about_the_arm_after_it() {
    check(
        r#"<?php
function arms(int $n): string
{
    switch ($n) {
        case 1:
            $n = 1;
            break;
        case 2:
            $n = 2;
            break;
    }
    return 'ok';
}
"#,
        expect![["<everything is reachable>"]],
    );
}

#[test]
fn a_statement_after_a_break_in_the_same_arm_is_dead() {
    check(
        r#"<?php
function arms(int $n): string
{
    switch ($n) {
        case 1:
            break;
            $n = 1;
    }
    return 'ok';
}
"#,
        expect![[r#"
            7:13-7:20 $n = 1;
        "#]],
    );
}

#[test]
fn a_throw_and_an_exit_end_a_block_as_surely_as_a_return() {
    check(
        r#"<?php
function thrown(int $n): string
{
    throw new \RuntimeException('no');
    $n = 1;
}

function stopped(int $n): string
{
    exit(1);
    $n = 1;
}
"#,
        expect![[r#"
            5:5-5:12 $n = 1;
            11:5-11:12 $n = 1;
        "#]],
    );
}

#[test]
fn a_declaration_written_after_a_return_is_hoisted_and_is_not_reported() {
    check(
        r#"<?php
function guard(): string
{
    return 'ok';
}

return 'done';

function hoisted(): string
{
    return 'later';
}
"#,
        expect![["<everything is reachable>"]],
    );
}

#[test]
fn a_run_stops_at_a_declaration_php_hoists() {
    // The fixture this test was written with used an `echo`, which is not a declaration and does
    // not hoist -- so it pinned a gap rather than the rule its name describes. A `function` written
    // after a `return` really is reachable, because PHP hoists it, and so is anything below it.
    check(
        r#"<?php
function tail(int $n): string
{
    return 'ok';
    $n = 1;
    function hoisted(): void {}
    $n = 2;
}
"#,
        expect![[r#"
            5:5-5:12 $n = 1;
        "#]],
    );
}

#[test]
fn a_run_carries_on_through_an_echo() {
    // The shape a dead region usually starts with. An `echo` is unmodelled, and while every
    // unmodelled statement ended the run this reported the assignment above it and lost the rest --
    // reading, from outside, as the check not working at all.
    check(
        r#"<?php
function tail(int $n): string
{
    return 'ok';
    $n = 1;
    echo $n;
    $n = 2;
}
"#,
        expect![[r#"
            5:5-7:12 $n = 1; echo $n; $n = 2;
        "#]],
    );
}

#[test]
fn dead_code_inside_a_closure_is_found_and_dead_code_around_one_swallows_it() {
    check(
        r#"<?php
function callbacks(array $rows): array
{
    $map = function (int $row): int {
        return $row;
        $row = 1;
    };
    return \array_map($map, $rows);
}

function inside_dead_code(array $rows): array
{
    return $rows;
    $map = function (int $row): int {
        return $row;
        $row = 1;
    };
}
"#,
        expect![[r#"
            6:9-6:18 $row = 1;
            14:5-17:7 $map = function (int $row): int { return $row; $row = 1; };
        "#]],
    );
}

#[test]
fn a_loop_that_can_end_leaves_the_code_after_it_reachable() {
    check(
        r#"<?php
function loops(array $rows): string
{
    foreach ($rows as $row) {
        return 'first';
    }
    return 'none';
}

function endless(): string
{
    while (true) {
        \work();
    }
    return 'never';
}
"#,
        expect![[r#"
            15:5-15:20 return 'never';
        "#]],
    );
}

#[test]
fn a_docblock_above_dead_code_is_not_itself_the_dead_thing() {
    check(
        r#"<?php
function annotated(): string
{
    return 'ok';
    /** @var int $n */
    $n = 1;
}
"#,
        expect![[r#"
            6:5-6:12 $n = 1;
        "#]],
    );
}
