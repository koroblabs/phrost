//! `$x = match ($k) { ... }` -- the value a `match` produces.
//!
//! The same union `??` and the ternary already produce, generalised to as many operands as there
//! are arms: what can be reached through the result is what every surviving arm agrees on. A
//! `throw` arm is not a surviving arm -- it removes itself rather than joining the union and
//! forcing every other arm's class to share something with an exception it never actually becomes.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_match_produces_a_union_every_arm_agrees_on() {
    check_view(
        r#"
//- /src/app.php
<?php
class Worker
{
    public function shared(): void {}
    public function onlyWorker(): void {}
}
class Backup
{
    public function shared(): void {}
}
class Fallback
{
    public function shared(): void {}
}

function pick(int $k): void
{
    $x = match ($k) {
        1 => new Worker(),
        2 => new Backup(),
        default => new Fallback(),
    };
    $x->sh@here@ared();
}
"#,
        &[Query::cursor("member reached through a match", "here")],
        expect![[r#"
            member reached through a match
            - member access Worker::shared [method]
        "#]],
    );
}

#[test]
fn a_throw_arm_does_not_widen_what_the_union_requires() {
    // Every real arm here declares `shared`, but the exception the default arm throws does not.
    // Reading the `throw` as a third operand would mean no class declares `shared` on every
    // operand, and the access would resolve to nothing at all -- so this also pins that the arm
    // was excluded rather than merely tolerated.
    check_view(
        r#"
//- /src/app.php
<?php
class Worker
{
    public function shared(): void {}
}
class Backup
{
    public function shared(): void {}
}

function pick(int $k): void
{
    $x = match ($k) {
        1 => new Worker(),
        2 => new Backup(),
        default => throw new \RuntimeException('unreachable'),
    };
    $x->sh@here@ared();
}
"#,
        &[Query::cursor("member reached past a throw arm", "here")],
        expect![[r#"
            member reached past a throw arm
            - member access Worker::shared [method]
        "#]],
    );
}

#[test]
fn every_arm_naming_the_same_class_collapses_to_it() {
    // Three arms agreeing on one class are not a three-way choice: `Repo|Repo|Repo` is not a type
    // anyone wrote, and the variable's class is read as plain `Repo`.
    check_view(
        r#"
//- /src/app.php
<?php
class Repo
{
    public function find(): void {}
}

function pick(int $k): void
{
    $one = match ($k) {
        1 => new Repo(),
        2 => new Repo(),
        default => new Repo(),
    };
    $on@here@e;
}
"#,
        &[Query::variable_class(
            "same class on every arm",
            "here",
            "one",
        )],
        expect![[r#"
            same class on every arm
            - Repo
        "#]],
    );
}

#[test]
fn a_subjectless_match_with_one_surviving_arm_is_read_as_that_arms_class() {
    // `match (true) { ... }` is the guard-chain idiom, and with every arm but one a `throw` there
    // is exactly one value left -- the class it holds, not a one-operand union wrapping it.
    check_view(
        r#"
//- /src/app.php
<?php
class Repo
{
    public function find(): void {}
}

function pick(int $ok): void
{
    $one = match (true) {
        $ok > 0 => new Repo(),
        default => throw new \RuntimeException('unreachable'),
    };
    $on@here@e;
}
"#,
        &[Query::variable_class(
            "a subjectless match with one survivor",
            "here",
            "one",
        )],
        expect![[r#"
            a subjectless match with one survivor
            - Repo
        "#]],
    );
}
