//! `$a ?? $b`, the ternary, and `match` — a value the code wrote several of.
//!
//! One value out of several, which is a union — so the path keeps every operand and lets the layer
//! that can resolve them decide what they agree on. `match` is the N-ary case: its arms are the
//! same shape as a ternary's two, just more of them, and a `throw` arm is not one of them at all.

use expect_test::expect;

use super::check;

#[test]
fn a_coalesce_keeps_both_operands() {
    check(
        r#"
<?php
function maybeWorker() {}

function run(): void
{
    $x = maybeWorker() ?? new Fallback();
    $x;
    $0
}
"#,
        "x",
        expect!["maybeWorker() ?? new Fallback"],
    );
}

#[test]
fn a_ternary_keeps_both_arms() {
    check(
        r#"
<?php
function run(bool $c): void
{
    $x = $c ? new Foo() : new Bar();
    $x;
    $0
}
"#,
        "x",
        expect!["new Foo or new Bar"],
    );
}

#[test]
fn a_short_ternary_reuses_the_condition_as_its_left_operand() {
    // `?:` has no then-branch, and the value it produces when the condition was truthy is the
    // condition itself.
    check(
        r#"
<?php
function maybeWorker() {}

function run(): void
{
    $x = maybeWorker() ?: new Fallback();
    $x;
    $0
}
"#,
        "x",
        expect!["maybeWorker() ?: new Fallback"],
    );
}

#[test]
fn a_null_arm_travels_beside_the_other_rather_than_over_it() {
    // `$x = $c ? $this->find() : NULL` is how a nullable is produced when there is no `?->` to
    // write, and it is most of the ternaries in a real project. The null is an operand like any
    // other here; folding `T|null` into `?T` is the reading layer's job, because only it knows what
    // the other arm resolved to.
    check(
        r#"<?php
class Repo
{
    public function run(bool $ok): void
    {
        $one = $ok ? $this->find() : NULL;
        $o$0ne;
    }
}
"#,
        "one",
        expect!["$this->find() or (null)"],
    );
}

#[test]
fn a_match_keeps_every_arm_including_the_default() {
    // The default is not a special case here: it writes a body like any other arm, and that body
    // is the only thing the value side reads. Several conditions on one arm (`2, 3 => ...`) are
    // still one operand -- the conditions themselves never appear on this side.
    check(
        r#"
<?php
function run(int $k): void
{
    $x = match ($k) {
        1 => new Foo(),
        2, 3 => new Bar(),
        default => new Baz(),
    };
    $x;
    $0
}
"#,
        "x",
        expect!["new Foo or new Bar or new Baz"],
    );
}

#[test]
fn a_subjectless_match_is_not_special_cased_into_nothing() {
    // `match (true) { ... }` is a guard chain, not a construct with no arms: the subject is just
    // `true`, and the conditions being comparisons rather than plain values changes nothing about
    // which arm produced the value.
    check(
        r#"
<?php
function run(int $k): void
{
    $x = match (true) {
        $k > 5 => new Foo(),
        $k > 1 => new Bar(),
        default => new Baz(),
    };
    $x;
    $0
}
"#,
        "x",
        expect!["new Foo or new Bar or new Baz"],
    );
}

#[test]
fn a_throw_arm_removes_itself_rather_than_joining_the_union() {
    // The guarded-`match` idiom: every case but one is an escape, and reading the escape as a real
    // operand would leave the union naming a type nothing ever holds.
    check(
        r#"
<?php
function run(int $k): void
{
    $x = match ($k) {
        1 => new Foo(),
        2 => new Bar(),
        default => throw new RuntimeException(),
    };
    $x;
    $0
}
"#,
        "x",
        expect!["new Foo or new Bar"],
    );
}

#[test]
fn one_surviving_arm_is_that_arms_value_and_not_a_one_operand_union() {
    // With every other arm a `throw`, one operand is left -- and it has to come back exactly as
    // that operand would on its own, steps included, rather than as a union wrapping a single
    // member.
    check(
        r#"<?php
class Repo
{
    public function run(int $k): void
    {
        $one = match ($k) {
            1 => $this->find(),
            default => throw new RuntimeException(),
        };
        $o$0ne;
    }
}
"#,
        "one",
        expect!["$this->find()"],
    );
}

#[test]
fn a_match_whose_every_arm_throws_gives_the_value_no_path() {
    // Nothing here ever hands the assignment a type, and that has to read as "no path", not as a
    // union of zero operands.
    check(
        r#"
<?php
function run(int $k): void
{
    $x = match ($k) {
        1 => throw new RuntimeException(),
        default => throw new RuntimeException(),
    };
    $x;
    $0
}
"#,
        "x",
        expect!["<not declared here>"],
    );
}
