//! The calls before a position that were made *on* a value, beside the ones it was handed to.
//!
//! A method may say what it leaves in its receiver (`@phpstan-self-out`), and that reaches the
//! variable through the same walk a `@param-out` does -- so the walk reports a receiver as one more
//! way of being handed over. Only for a method name the filter admits, which is how the index keeps
//! a value that is merely called on out of the walk; and only for a receiver that keeps what a call
//! leaves in it, which a value reached through another call does not.

use expect_test::expect;
use phrost_model::{FileId, Interner, TextSize};

use crate::{Callee, PassedAs, PassedPaths, ValuePath, local_path, passed_to_calls};

fn calls_on(source: &str, variable: &str, leaves_receiver: &dyn Fn(&str) -> bool) -> String {
    let offset = TextSize::try_from(source.find("$0").expect("the source marks a position"))
        .expect("fixture fits in u32");
    let text = source.replace("$0", "");
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        &text,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(lowered.errors.is_empty(), "fixture does not parse");
    let hir = &lowered.hir;
    let subject: ValuePath = local_path(&interner.name(variable));
    let index = PassedPaths::of(hir, leaves_receiver);
    let found = passed_to_calls(
        hir,
        offset,
        &subject,
        index.statements(&subject),
        leaves_receiver,
    );
    let lines = found
        .iter()
        .map(|call| {
            let callee = match &call.callee {
                Callee::Member { name, .. } => format!("{name}()"),
                Callee::Function(name) => format!("{}()", hir.name_ref(*name).path),
                Callee::Constructed(_) => "new".to_owned(),
            };
            match call.passed_as {
                PassedAs::Receiver(_) => format!("the receiver of {callee}"),
                PassedAs::Argument(position) => format!("argument {position} of {callee}"),
            }
        })
        .collect::<Vec<_>>();
    match lines.is_empty() {
        true => "<no calls>".to_owned(),
        false => lines.join("\n"),
    }
}

#[test]
fn a_call_on_the_value_is_reported_when_its_name_may_change_the_receiver() {
    let answer = calls_on(
        r#"<?php
function run($box, $next): void
{
    $box->replace($next);
    $box->describe();
    takes($box);
    $b$0ox;
}
"#,
        "box",
        &|name| name == "replace",
    );
    expect![[r#"
        the receiver of replace()
        argument 0 of takes()"#]]
    .assert_eq(&answer);
}

/// `$repo->items()` is whatever `items()` handed back that time, so a call made on it changed that
/// object and says nothing about `$repo` -- nor about the next `$repo->items()`.
#[test]
fn a_call_on_a_value_reached_through_a_call_is_not_one_on_the_variable() {
    let answer = calls_on(
        r#"<?php
function run($repo, $next): void
{
    $repo->items()->replace($next);
    $re$0po;
}
"#,
        "repo",
        &|name| name == "replace",
    );
    expect!["<no calls>"].assert_eq(&answer);
}
