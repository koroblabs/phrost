//! `in_array($x, [<non-object values>], true)`, a strict membership test read as a discriminator.
//!
//! The same shape `is_string` is: the file cannot say which scalar kind a class constant actually
//! holds, but it can say the haystack names no class, and "not an object" is enough to remove the
//! one alternative of a union that is.

use expect_test::expect;

use super::check;

#[test]
fn a_strict_membership_test_narrows_inside_the_branch() {
    check(
        r#"<?php
class Status
{
    const ACTIVE = 'active';
    const INACTIVE = 'inactive';
}
class Repo
{
    public function run($value): void
    {
        if (in_array($value, [Status::ACTIVE, Status::INACTIVE], true)) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["$value an element of list literal of 2"],
    );
}

#[test]
fn a_strict_membership_test_narrows_after_a_leaving_guard() {
    check(
        r#"<?php
class Status
{
    const ACTIVE = 'active';
    const INACTIVE = 'inactive';
}
class Repo
{
    public function run($value): void
    {
        if (in_array($value, [Status::ACTIVE, Status::INACTIVE], true)) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["(object) $value"],
    );
}

#[test]
fn string_literals_in_the_haystack_narrow_the_same_way() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (in_array($value, ['active', 'inactive'], true)) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["(object) $value"],
    );
}

#[test]
fn a_loose_comparison_proves_nothing() {
    // PHP's loose `in_array` can equate an `int` with a numeric string, so a haystack of scalars
    // says nothing about whether the value is an object -- the same trap `FalseTest` refuses for
    // `== false`.
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (in_array($value, [1, 2], false)) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["inherited parameter 0"],
    );
}

#[test]
fn a_haystack_with_an_unrecognized_element_proves_nothing() {
    // `expensiveLookup()` might return an object, and nothing here resolves what it returns -- so
    // the haystack cannot be said to name no class, and the honest answer is silence.
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (in_array($value, [expensiveLookup(), 'literal'], true)) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["inherited parameter 0"],
    );
}
