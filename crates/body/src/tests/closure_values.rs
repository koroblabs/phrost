//! A closure read as a value, and calling the variable that holds one.
//!
//! The half of `$make = fn (): Query => ...; $make()` this layer owns: the closure is a value with a
//! path of its own, and the call applies an [`crate::PathStep::Invoked`] to it. What the closure
//! gives back is deliberately *not* settled here -- a declared return type is a reference into this
//! file's arena and an arrow function that declares none says so only through the value its body
//! holds, and neither is a question a body answers.

use expect_test::expect;

use super::check;

#[test]
fn a_closure_is_a_value_with_a_path() {
    // The declared return travels with it, which is the whole of what makes the variable worth
    // resolving: without it `$make` is a `Closure` and nothing more.
    check(
        r#"<?php
function go(): void
{
    $make = fn (): Query => new Query();
    $ma$0ke;
}
"#,
        "make",
        expect!["closure(): Query"],
    );
}

#[test]
fn the_long_form_reads_the_same_as_the_arrow() {
    // Two spellings of one thing. `function () use (...)` is lowered to the same node with a
    // different `kind`, and nothing about reading it as a value depends on which.
    check(
        r#"<?php
function go(): void
{
    $make = function (): Query {
        return new Query();
    };
    $ma$0ke;
}
"#,
        "make",
        expect!["closure(): Query"],
    );
}

#[test]
fn a_closure_that_declares_nothing_is_still_a_value() {
    // The path exists either way. An arrow function's body is one expression and the layer above
    // reads it; a `function () { ... }` says nothing at all -- and both are a callable, which is
    // more than the nothing this used to answer.
    check(
        r#"<?php
function go(): void
{
    $make = fn () => new Query();
    $ma$0ke;
}
"#,
        "make",
        expect!["closure"],
    );
}

#[test]
fn calling_the_variable_applies_an_invoked_step() {
    // The shape the whole feature exists for. The step is what says a call happened; the root is
    // what says a closure is being called rather than an object with `__invoke`.
    check(
        r#"<?php
function go(): void
{
    $make = fn (): Query => new Query();
    $q = $make();
    $$0q;
}
"#,
        "q",
        expect!["$make()"],
    );
}

#[test]
fn a_closure_invoked_where_it_is_written_needs_no_variable() {
    // `(fn (): Query => ...)()` -- the callee of the indirect call is the literal itself, so the
    // root and the step are one expression apart with nothing in between to hold the value.
    check(
        r#"<?php
function go(): void
{
    $q = (fn (): Query => new Query())();
    $$0q;
}
"#,
        "q",
        expect!["closure(): Query()"],
    );
}

#[test]
fn a_closure_passed_to_a_call_is_still_read_as_one() {
    // The argument position, which is where most closures in real code are written. The path is the
    // same one; what differs is only who asks for it.
    check(
        r#"<?php
function go(): void
{
    $make = fn (): Query => new Query();
    $rows = array_map($make, []);
    $ma$0ke;
}
"#,
        "make",
        expect!["closure(): Query"],
    );
}
