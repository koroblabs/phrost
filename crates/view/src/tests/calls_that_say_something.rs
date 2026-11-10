//! Of the calls a value is handed to, the ones whose callee says anything about it.
//!
//! A call statement is read for three things -- a by-reference parameter, an `@assert` about the
//! parameter the value filled, a contract on the receiver -- and nearly every call a body writes
//! is none of them. Those used to be looked up again by every later read of the value: a local
//! handed to four hundred calls took 100 ms to check in a body where not one call says anything.
//! What a callee says is settled once per path now, and a read is handed only the calls that
//! speak. See `IndexView::passed_to_calls`.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;

use crate::{IndexView, view::FOLLOW_LIMIT};

/// The calls before the marker that `$value` was handed to and that say something about it, as
/// the names of what was called.
fn check(body: &str, expect: Expect) {
    let fixture = format!(
        r#"
//- /src/app.php
<?php
namespace App;

function takes(mixed $given): void {{}}

/** @phpstan-assert !null $given */
function sure(mixed $given): void {{}}

/** @phpstan-assert !null $second */
function sure_of_the_second(mixed $first, mixed $second): void {{}}

function fills(?array &$given): void {{}}

function run(mixed $value, mixed $other): void
{{
{body}
    $val@here@ue;
}}
"#
    );
    let spec = FixtureSpec::parse(&fixture);
    let world = World::build(&spec);
    let position = spec.markers().position("here");
    let file = world.file_id(&position.path);

    let answer = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let hir = view.current().expect("the cursor file is open").hir();
        let value = phrost_body::local_path(
            &world
                .skeleton()
                .interner()
                .lookup_name("value")
                .expect("the fixture names it"),
        );
        let found = view
            .passed_to_calls(hir, position.offset, &value, FOLLOW_LIMIT)
            .iter()
            .map(|call| match &call.callee {
                phrost_body::Callee::Function(name) => format!("{}()\n", hir.name_ref(*name).path),
                phrost_body::Callee::Member { name, .. } => format!("{name}()\n"),
                phrost_body::Callee::Constructed(_) => "new\n".to_owned(),
            })
            .collect::<String>();
        match found.is_empty() {
            true => "<no call says anything>\n".to_owned(),
            false => found,
        }
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&answer);
}

/// Handed over three times, to a function that declares nothing about what it takes.
#[test]
fn a_call_that_declares_nothing_about_its_argument_is_not_read() {
    check(
        "    takes($value);\n    takes($value);\n    takes($value);",
        expect![[r#"
            <no call says anything>
        "#]],
    );
}

/// An `@assert` about the parameter the value filled, between two calls that say nothing.
#[test]
fn a_call_that_asserts_about_the_argument_is_read() {
    check(
        "    takes($value);\n    sure($value);\n    takes($value);",
        expect![[r#"
            sure()
        "#]],
    );
}

/// The tag names the *second* parameter, and the value filled the first: a call that asserts,
/// about something else.
#[test]
fn an_assertion_about_another_parameter_is_not_about_this_value() {
    check(
        "    sure_of_the_second($value, $other);",
        expect![[r#"
            <no call says anything>
        "#]],
    );
}

/// A by-reference parameter is the callee saying it may write the caller's variable.
#[test]
fn a_call_that_takes_the_argument_by_reference_is_read() {
    check(
        "    takes($value);\n    fills($value);",
        expect![[r#"
            fills()
        "#]],
    );
}
