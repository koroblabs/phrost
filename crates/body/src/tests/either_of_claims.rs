//! A disjunction whose operands are not all `instanceof`.
//!
//! `$x instanceof A || $x instanceof B` had a rule; `is_string($x) || $x instanceof Foo` did not,
//! and neither did its De Morgan twin `!is_string($x) && !$x instanceof Foo` past a `throw` --
//! which is how an early-return guard with two alternatives is written.
//!
//! One gap with two faces, and a matrix is what showed it was one: a conjunction narrowed *inside*
//! its branch and a disjunction narrowed *after* a leaving one, while the other diagonal narrowed
//! nothing. Both failures are the same claim, because the negation of a conjunction of negations is
//! the disjunction.

use expect_test::expect;

use super::check;

#[test]
fn a_predicate_or_an_instanceof_narrows_to_the_union_of_both() {
    check(
        r#"<?php
class Foo {}

class Repo
{
    public function run(string|Foo|int $x): void
    {
        if (is_string($x) || $x instanceof Foo) {
            $v$0alue = $x;
        }
    }
}
"#,
        "x",
        expect!["(string) $x or instanceof Foo $x"],
    );
}

#[test]
fn the_same_union_past_a_guard_that_leaves() {
    check(
        r#"<?php
class Foo {}

class Repo
{
    public function run(string|Foo|int $x): void
    {
        if (!is_string($x) && !$x instanceof Foo) {
            throw new \RuntimeException('');
        }
        $v$0alue = $x;
    }
}
"#,
        "x",
        expect!["(string) $x or instanceof Foo $x"],
    );
}

#[test]
fn one_operand_that_says_nothing_gives_up_the_whole_disjunction() {
    // A union built from the operands that *did* answer is narrower than the truth: the branch can
    // be reached by the one nothing was proved about. The rule `list_literal_element` states for
    // entries, here for alternatives.
    check(
        r#"<?php
class Foo {}

class Repo
{
    public function run(string|Foo|int $x, bool $other): void
    {
        if (is_string($x) || $other) {
            $v$0alue = $x;
        }
    }
}
"#,
        "x",
        expect!["<union>"],
    );
}
