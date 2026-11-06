//! What a guard proves, and where it stops proving it.
//!
//! Both polarities of both connectives have a test here, because taking the wrong one of the four
//! means naming a class the value is not -- and De Morgan decides which is which.

use expect_test::expect;

use super::check;

#[test]
fn an_instanceof_guard_narrows_inside_its_branch() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if ($value instanceof Alpha) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["instanceof Alpha $value"],
    );
}

#[test]
fn a_guard_does_not_reach_past_its_branch() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if ($value instanceof Alpha) {
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
fn a_guard_reached_through_an_and_still_holds() {
    // Both halves of an `&&` hold inside the branch, so the contract may be either one.
    check(
        r#"<?php
class Repo
{
    public function run($value, $other): void
    {
        if ($other && $value instanceof Alpha) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["instanceof Alpha $value"],
    );
}

#[test]
fn a_guard_reached_through_an_or_proves_nothing() {
    // `$value instanceof Alpha || $other` is true whenever `$other` is, so inside the branch
    // `$value` may be anything. Narrowing here would name a class the value is not.
    check(
        r#"<?php
class Repo
{
    public function run($value, $other): void
    {
        if ($value instanceof Alpha || $other) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["inherited parameter 0"],
    );
}

#[test]
fn the_innermost_guard_wins() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if ($value instanceof Alpha) {
            if ($value instanceof Beta) {
                $val$0ue;
            }
        }
    }
}
"#,
        "value",
        expect!["instanceof Beta $value"],
    );
}

#[test]
fn an_assignment_after_a_guard_replaces_what_it_proved() {
    // The guard held when it was written; the assignment happened after. Keeping the narrowing would
    // describe a value that has been replaced.
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if ($value instanceof Alpha) {
            $value = new Beta();
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["new Beta"],
    );
}

#[test]
fn a_guard_after_an_assignment_wins() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $value = new Beta();
        if ($value instanceof Alpha) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["instanceof Alpha $value"],
    );
}

#[test]
fn an_early_return_guard_narrows_the_code_after_it() {
    // How a lot of Drupal is written, and it only works because the branch cannot fall through.
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (!($value instanceof Alpha)) {
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
fn a_guard_that_throws_narrows_just_as_well() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (!($value instanceof Alpha)) {
            throw new \LogicException();
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
fn a_guard_whose_branch_falls_through_narrows_nothing_after_it() {
    // Without the `return` the branch is just a branch, and after the `}` the value may be anything
    // again. Narrowing here would be the difference between reading the code and assuming it.
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (!($value instanceof Alpha)) {
            $value;
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
fn an_assert_narrows_the_rest_of_the_scope() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        assert($value instanceof Alpha);
        $val$0ue;
    }
}
"#,
        "value",
        expect!["instanceof Alpha $value"],
    );
}

#[test]
fn a_guard_on_a_negated_and_still_proves_its_instanceof() {
    // The guard leaves when `!(A && B)` holds, so past it `A && B` does -- and both halves of an
    // `&&` hold. Two sign flips, which is why polarity is tracked rather than guessed at.
    check(
        r#"<?php
class Repo
{
    public function run($value, $other): void
    {
        if (!($value instanceof Alpha && $other)) {
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
fn a_guard_on_an_or_rules_out_both_of_its_operands() {
    // The guard leaves when `A || B` holds, so past it neither does. Both claims survive, and "not
    // an `Alpha`" is one of them -- what it leaves depends on what the declaration listed, which is
    // the reader's question. Taking `Alpha` itself here would be exactly backwards.
    check(
        r#"<?php
class Repo
{
    public function run($value, $other): void
    {
        if ($value instanceof Alpha || $other) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["not instanceof Alpha $value"],
    );
}

#[test]
fn an_else_that_leaves_narrows_from_the_other_side() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if ($value instanceof Alpha) {
        } else {
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

/// `if (($x = $this->find()) && $x instanceof Special)` -- get it, and use it if it is the kind I
/// need.
///
/// Both halves of a conjunction hold, so both claims are kept and the second is about the value the
/// first left. Ranking them was wrong in both directions: taking the left one meant the assignment's
/// "it is not null" won over the `instanceof` and every call to the narrowed class's own API read as
/// undefined, and taking the stronger one dropped the null-safety of `if ($v && !is_array($v))`.
#[test]
fn a_conjunction_keeps_both_halves_of_what_it_proves() {
    check(
        r#"<?php
class Manager
{
    public function run(): void
    {
        if (($carrier = $this->find()) && $carrier instanceof Cancellable) {
            $carr$0ier;
        }
    }
}
"#,
        "carrier",
        expect!["instanceof Cancellable not null $this->find()"],
    );
}

/// The same the other way round is *not* the same claim, and this is where that shows.
///
/// The order the two are written in decides what they are about: with the assignment first, the
/// `instanceof` describes the value it produced and the two compose. With the assignment second,
/// the `instanceof` tested whatever `$carrier` held *before* it, and nothing carries across a
/// rebinding -- what is left is the truthiness of what was just assigned. This used to answer
/// `not null instanceof Cancellable $carrier`, which claims a class for a value the guard never
/// saw.
#[test]
fn a_conjunction_does_not_carry_a_claim_across_a_rebinding() {
    check(
        r#"<?php
class Manager
{
    public function run(): void
    {
        if ($carrier instanceof Cancellable && ($carrier = $this->find())) {
            $carr$0ier;
        }
    }
}
"#,
        "carrier",
        expect!["not null $this->find()"],
    );
}

/// A conjunction proving only the weaker thing still proves it.
#[test]
fn a_conjunction_without_a_class_still_says_what_it_can() {
    check(
        r#"<?php
class Manager
{
    public function run(?Repo $repo): void
    {
        if ($repo && $repo->isReady()) {
            $re$0po;
        }
    }
}
"#,
        "repo",
        expect!["asserted by isReady() if true"],
    );
}
