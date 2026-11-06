//! `in_array($x, $list, true)` that held: the needle is one of the list's elements.
//!
//! The claim names the container rather than a type, because what its elements are is a
//! declaration this layer does not read. Only strict, and only the side that held: a needle that
//! was not found may be anything, the elements' own type included.

use expect_test::expect;

use super::check;

#[test]
fn a_strict_search_that_held_names_the_container() {
    check(
        r#"<?php
function run(?User $user, array $users): void
{
    if (in_array($user, $users, true)) {
        $us$0er;
    }
}
"#,
        "user",
        expect!["$user an element of $users"],
    );
}

#[test]
fn a_strict_search_past_a_leaving_guard_names_the_container() {
    check(
        r#"<?php
function run(?User $user, array $users): void
{
    if (!in_array($user, $users, true)) {
        return;
    }
    $us$0er;
}
"#,
        "user",
        expect!["$user an element of $users"],
    );
}

#[test]
fn a_strict_search_that_failed_proves_nothing() {
    // Not being one of the admins says nothing about what the value is.
    check(
        r#"<?php
function run(?User $user, array $users): void
{
    if (!in_array($user, $users, true)) {
        $us$0er;
    }
}
"#,
        "user",
        expect!["?User"],
    );
}

#[test]
fn a_loose_search_proves_nothing() {
    // `in_array('1', [1])` holds, so a loose search says nothing about the needle's type.
    check(
        r#"<?php
function run(?User $user, array $users): void
{
    if (in_array($user, $users)) {
        $us$0er;
    }
}
"#,
        "user",
        expect!["?User"],
    );
}
