//! `foreach ($ints as &$item) { $item = ...; }` writes into `$ints`, with no assignment to it
//! anywhere in the text.
//!
//! So past the loop the container's binding is the loop: what it held at the header, beside every
//! whole write the body made to the reference. Only a loop control certainly went through, and only
//! one whose body writes the reference and not the container.

use expect_test::expect;

use super::check;

#[test]
fn a_loop_that_wrote_through_the_reference_binds_the_container() {
    check(
        r#"<?php
function run(array $ints): void
{
    foreach ($ints as &$item) {
        $item = 'x';
        $item .= 'y';
    }
    unset($item);
    $in$0ts;
}
"#,
        "ints",
        expect!["the elements of $ints before, or (string) or (non-falsy-string)"],
    );
}

#[test]
fn a_write_that_names_no_value_is_carried_as_such() {
    check(
        r#"<?php
function run(array $pairs): void
{
    foreach ($pairs as &$pair) {
        [$pair, $rest] = $pair;
    }
    $pa$0irs;
}
"#,
        "pairs",
        expect!["the elements of $pairs before, or a write nothing names"],
    );
}

#[test]
fn a_loop_that_wrote_into_an_element_is_not_read() {
    // `$row['seen'] = true` changes what one slot holds, not which slots the container has.
    check(
        r#"<?php
function run(array $rows): void
{
    foreach ($rows as &$row) {
        $row['seen'] = true;
    }
    $ro$0ws;
}
"#,
        "rows",
        expect!["array"],
    );
}

#[test]
fn a_loop_by_value_writes_nothing_back() {
    check(
        r#"<?php
function run(array $ints): void
{
    foreach ($ints as $item) {
        $item = 'x';
    }
    $in$0ts;
}
"#,
        "ints",
        expect!["array"],
    );
}

#[test]
fn a_loop_in_a_branch_is_not_read() {
    // It may not have run, and the container on the path around it is not what it was at the
    // loop's header.
    check(
        r#"<?php
function run(array $ints, bool $flag): void
{
    if ($flag) {
        foreach ($ints as &$item) {
            $item = 'x';
        }
    }
    $in$0ts;
}
"#,
        "ints",
        expect!["array"],
    );
}

#[test]
fn an_unset_inside_the_loop_ends_the_alias() {
    check(
        r#"<?php
function run(array $ints): void
{
    foreach ($ints as &$item) {
        unset($item);
        $item = 'x';
    }
    $in$0ts;
}
"#,
        "ints",
        expect!["array"],
    );
}

#[test]
fn an_assignment_after_the_loop_replaces_what_it_wrote() {
    check(
        r#"<?php
function run(array $ints): void
{
    foreach ($ints as &$item) {
        $item = 'x';
    }
    $ints = [];
    $in$0ts;
}
"#,
        "ints",
        expect!["(array)"],
    );
}
