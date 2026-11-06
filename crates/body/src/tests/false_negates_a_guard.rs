//! `false === <guard>`, where the guard is read backwards through the comparison.
//!
//! Different from [`super::false_guards`]: that file is about a value compared against `false`
//! directly, `$x === false`. Here the comparison is written around another guard entirely --
//! `false === is_string($x)`, `false === $x instanceof Y` -- and what it proves is that guard's
//! own claim, at the opposite polarity. `strpos($h, $n) !== false` already reads through `Not`
//! this way; `false === is_string($x)` is the same idiom with the operator spelled `===` instead.

use expect_test::expect;

use super::check;

#[test]
fn false_identity_on_instanceof_narrows_inside_the_branch() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (false === $value instanceof Alpha) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["not instanceof Alpha $value"],
    );
}

#[test]
fn false_identity_on_instanceof_narrows_after_a_leaving_guard() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (false === $value instanceof Alpha) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["instanceof Alpha $value"],
    );
}

#[test]
fn false_identity_on_a_type_predicate_narrows_inside_the_branch() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (false === is_string($value)) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["not (string) $value"],
    );
}

#[test]
fn false_identity_on_a_type_predicate_narrows_after_a_leaving_guard() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (false === is_string($value)) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["(string) $value"],
    );
}

#[test]
fn yoda_order_reads_the_same_as_the_guard_written_plainly() {
    // `$value instanceof Alpha === false` is the same claim with the operands the other way
    // round, and the `FalseTest` node the lowerer produces does not care which side wrote `false`.
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if ($value instanceof Alpha === false) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["instanceof Alpha $value"],
    );
}
