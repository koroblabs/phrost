//! A local read above everything that could have given it a value.
//!
//! One finding and a long tail of silences, and the silences are the check: PHP creates a variable
//! in a dozen places that do not look like writing, and a read of one it creates *below* the line
//! that reads it is legal in more of them still. Every test after the first three is about a shape
//! that must stay quiet.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner};

use crate::locals_read_before_bound;

/// Every early read the source makes, as `name` lines with both positions.
///
/// The whole result, positions included: a finding at the wrong occurrence of the right name is the
/// failure this check is most likely to have, and a name-only rendering would hide it.
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

    let mut out = String::new();
    for found in locals_read_before_bound(&lowered.hir, true) {
        out.push_str(&format!(
            "${} read at {:?}, bound at {:?}\n",
            found.name, found.range, found.bound_at
        ));
    }
    expect.assert_eq(&out);
}

#[test]
fn a_read_above_the_only_assignment_is_reported() {
    check(
        r#"<?php
function run(): int
{
    echo $total;
    $total = 1;
    return $total;
}
"#,
        expect![[r#"
            $total read at 37..43, bound at 49..55
        "#]],
    );
}

#[test]
fn only_the_first_read_of_a_name_is_reported() {
    // Four underlines say four mistakes. There is one: the assignment is in the wrong place.
    check(
        r#"<?php
function run(): int
{
    echo $total;
    echo $total;
    $total = 1;
    return $total;
}
"#,
        expect![[r#"
            $total read at 37..43, bound at 66..72
        "#]],
    );
}

#[test]
fn a_read_in_a_branch_above_the_assignment_is_reported() {
    // The branch is beside the point: whichever way control went, it went through the read first.
    check(
        r#"<?php
function run(bool $c): int
{
    if ($c) {
        return $count;
    }
    $count = 1;
    return $count;
}
"#,
        expect![[r#"
            $count read at 64..70, bound at 82..88
        "#]],
    );
}

#[test]
fn a_write_in_a_branch_above_the_read_is_not_reported() {
    // PHP warns when the branch is skipped, and this deliberately does not say so: an
    // `if`/`elseif`/`else` assigning in all three arms is the same shape to this walk, and telling
    // them apart needs an exhaustiveness answer nothing here has.
    check(
        r#"<?php
function run(bool $c): int
{
    if ($c) {
        $count = 1;
    }
    return $count;
}
"#,
        expect![""],
    );
}

#[test]
fn a_name_nothing_binds_at_all_belongs_to_the_other_check() {
    // Two reports for one name would be two mistakes where there is one, so this stays silent and
    // `locals_never_bound` speaks.
    check(
        r#"<?php
function run(): int
{
    return $missing;
}
"#,
        expect![""],
    );
}

#[test]
fn the_ways_php_binds_without_writing_are_all_quiet() {
    // Every one of these gives the name a value before the line that reads it, and none of them is
    // an assignment: a parameter, a `global`, a `static`, a `catch`, an annotation, a by-reference
    // argument, a destructuring, and a `foreach` header.
    check(
        r#"<?php
function run(int $given): mixed
{
    global $registry;
    static $seen;
    /** @var string $noted */
    $out = [$given, $registry, $seen, $noted];
    preg_match('/x/', 'y', $matches);
    $out[] = $matches;
    [$first, $second] = $out;
    list($third) = $out;
    ['k' => $keyed] = $out;
    $out[] = [$first, $second, $third, $keyed];
    try {
        $out[] = 1;
    } catch (Throwable $error) {
        $out[] = $error;
    }
    foreach ($out as $index => $one) {
        $out[] = [$index, $one];
    }
    return $out;
}
"#,
        expect![""],
    );
}

#[test]
fn the_constructs_that_exist_to_ask_are_quiet() {
    // `isset`, `empty` and `??` are all legal on a name nothing has assigned -- they are how PHP
    // asks whether it has been -- and `??=` and `++` write through the target they read.
    check(
        r#"<?php
function run(): mixed
{
    $out = [];
    if (isset($maybe)) {
        $out[] = $maybe;
    }
    if (empty($blank)) {
        $out[] = 1;
    }
    $out[] = $fallback ?? 'default';
    $counted ??= 0;
    $out[] = $counted;
    $ticks++;
    $out[] = $ticks;
    $maybe = 1;
    $blank = 1;
    $fallback = 1;
    return $out;
}
"#,
        expect![""],
    );
}

#[test]
fn a_write_later_in_the_same_loop_is_quiet() {
    // The write reaches the read on the pass after, which is what a carry-over variable is for.
    check(
        r#"<?php
function run(array $rows): array
{
    $out = [];
    foreach ($rows as $row) {
        $out[] = [$row, $previous];
        $previous = $row;
    }
    return $out;
}
"#,
        expect![""],
    );
}

#[test]
fn a_loop_header_read_after_its_body_writes_is_quiet() {
    // `while ($line !== false)` is evaluated again after the body assigned `$line`, so the loop
    // rule has to cover the whole statement and not only its body.
    check(
        r#"<?php
function run(): array
{
    $out = [];
    while ($line !== false) {
        $line = fgets(STDIN);
        $out[] = $line;
    }
    return $out;
}
"#,
        expect![""],
    );
}

#[test]
fn a_write_outside_the_loop_a_read_is_in_is_still_reported() {
    // The loop rule needs the binding to share the loop. One written below the whole loop cannot
    // reach a read inside it on any pass.
    check(
        r#"<?php
function run(array $rows): array
{
    $out = [];
    foreach ($rows as $row) {
        $out[] = $limit;
    }
    $limit = 10;
    return $out;
}
"#,
        expect![[r#"
            $limit read at 103..109, bound at 121..127
        "#]],
    );
}

#[test]
fn a_captured_name_is_the_enclosing_scopes_business() {
    // The scope that owns `$total` is in the same sweep and answers there. Asking inside the
    // closure would compare a position in a body that runs when something calls it.
    check(
        r#"<?php
function run(): callable
{
    $total = 1;
    return function () use ($total): int {
        return $total + 1;
    };
}
"#,
        expect![""],
    );
}

#[test]
fn a_by_reference_capture_may_write_the_outer_name_at_any_time() {
    check(
        r#"<?php
function run(): array
{
    $collect = function () use (&$rows): void {
        $rows[] = 1;
    };
    $collect();
    return $rows;
}
"#,
        expect![""],
    );
}

#[test]
fn a_scope_that_can_be_given_a_name_this_cannot_see_is_left_alone() {
    // The same refusal the never-bound check makes: `extract()` names variables with a string, so
    // no claim about the order of any name in the scope is safe.
    check(
        r#"<?php
function run(array $values): string
{
    extract($values);
    $found = $whatever;
    $whatever = 'x';
    return $found;
}
"#,
        expect![""],
    );
}

#[test]
fn a_superglobal_and_the_engines_own_variables_are_quiet() {
    check(
        r#"<?php
function run(): array
{
    return [$_GET, $GLOBALS, $argv, $argc];
}
"#,
        expect![""],
    );
}

#[test]
fn an_element_write_is_what_creates_the_variable() {
    // `$rows['a'] = 1` creates `$rows`, so the read below it is not early -- and the read of `$key`
    // inside the target is a read, not a write, which is why the target's shape is followed rather
    // than its range.
    check(
        r#"<?php
function run(string $key): array
{
    $rows[$key] = 1;
    return $rows;
}
"#,
        expect![""],
    );
}
