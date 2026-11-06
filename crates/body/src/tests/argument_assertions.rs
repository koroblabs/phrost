//! What a guard proves about one of a call's *arguments*, rather than about its receiver.
//!
//! `Validator::isUser($x)` and `$validator->isUser($x)` are the mirror of `$x->isX()`: neither
//! call is made on `$x`, so the recognition here has to look among the arguments instead of at
//! the receiver -- and, for a static call, there is no receiver at all to have gotten this from by
//! accident. Every test checks the *branch's* polarity that gets captured (`asserted if …`), never
//! a docblock tag's -- that comparison happens only once, at resolution, which is what keeps a `!`
//! in front of the call from narrowing the wrong side.

use expect_test::expect;

use super::check;

#[test]
fn a_static_call_narrows_by_its_argument_inside_the_true_branch() {
    check(
        r#"<?php
class Validator
{
    public function run($input): void
    {
        if (Validator::isUser($input)) {
            $inp$0ut;
        }
    }
}
"#,
        "input",
        expect!["argument 0 of isUser(), asserted if true, was $input"],
    );
}

#[test]
fn an_instance_call_narrows_by_its_argument_the_same_way() {
    check(
        r#"<?php
class Consumer
{
    public function run($validator, $input): void
    {
        if ($validator->isUser($input)) {
            $inp$0ut;
        }
    }
}
"#,
        "input",
        expect!["argument 0 of isUser(), asserted if true, was $input"],
    );
}

/// The receiver form wins when both are true -- `$x->isX($x)` is still about the receiver, not
/// about the argument that happens to be the same value.
#[test]
fn the_receiver_form_is_tried_first_when_a_call_is_made_on_the_same_value_it_is_passed() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if ($value->isReady($value)) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["asserted by isReady() if true"],
    );
}

/// A `!` in front of the call flips the *branch's* polarity, and nothing else -- the tag's own
/// polarity is a separate question, answered only at resolution.
#[test]
fn a_negated_call_narrows_by_its_argument_at_the_opposite_polarity() {
    check(
        r#"<?php
class Validator
{
    public function run($input): void
    {
        if (!Validator::isUser($input)) {
            $inp$0ut;
        }
    }
}
"#,
        "input",
        expect!["argument 0 of isUser(), asserted if false, was $input"],
    );
}

/// A leaving guard proves the condition false for the code after it, the same as every other
/// truthiness guard here -- so the branch polarity captured is `false`, not `true`.
#[test]
fn a_leaving_guard_narrows_by_its_argument_for_the_rest_of_the_scope() {
    check(
        r#"<?php
class Guard
{
    public function run($input): void
    {
        if (Guard::isInvalid($input)) {
            return;
        }
        $inp$0ut;
    }
}
"#,
        "input",
        expect!["argument 0 of isInvalid(), asserted if false, was $input"],
    );
}

/// A spread passes an unknown number of values at unknown positions, so nothing here can say
/// which one -- if any -- the guarded value became. Giving up leaves the parameter's own
/// declaration, which is what a plain variadic already renders as.
#[test]
fn a_spread_argument_gives_up_rather_than_guessing_a_position() {
    check(
        r#"<?php
function run(mixed ...$rest): void
{
    if (Validator::isUser(...$rest)) {
        $re$0st;
    }
}
"#,
        "rest",
        expect!["variadic mixed"],
    );
}

/// `$x && Validator::isUser($x)` proves both halves, the same way `$x && $x->isSomething()`
/// already does for the receiver form -- so the argument-form claim nests on top of what the first
/// half already narrowed, rather than the two being ranked against each other.
#[test]
fn a_conjunction_composes_the_argument_form_with_what_came_before_it() {
    check(
        r#"<?php
class Validator
{
    public function run($input): void
    {
        if ($input !== null && Validator::isUser($input)) {
            $inp$0ut;
        }
    }
}
"#,
        "input",
        expect!["argument 0 of isUser(), asserted if true, was not null $input"],
    );
}
