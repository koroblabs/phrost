//! A literal handed to a call is not a value the next literal is.
//!
//! The index of which statement calls a path is handed to is keyed by the path, and a literal's
//! path is the type it produces: `Builtin(Int)` for every integer written in the file. So
//! `define('A', 1); define('B', 2);` read as one value handed to two calls, and resolving the
//! third literal walked both -- quadratic in a file that is a list of them, which a constants file
//! is. A value with no name is left out of the index; see `ValuePath::names_a_value`.

use expect_test::expect;
use phrost_model::{BuiltinTy, FileId, Interner, TextSize};

use crate::{Callee, PassedPaths, PathRoot, ValuePath, local_path, passed_to_calls};

const SOURCE: &str = r#"<?php
function run($value): void
{
    define('A', 1);
    takes(2);
    takes((int) $value);
    takes($value);
    $0;
}
"#;

fn handed_to(subject: impl Fn(&mut Interner) -> ValuePath) -> String {
    let offset = TextSize::try_from(SOURCE.find("$0").expect("the source marks a position"))
        .expect("fixture fits in u32");
    let text = SOURCE.replace("$0", "");
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        &text,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(lowered.errors.is_empty(), "fixture does not parse");
    let subject = subject(&mut interner);
    let hir = &lowered.hir;
    let index = PassedPaths::of(hir, &|_| false);
    let found = passed_to_calls(hir, offset, &subject, index.statements(&subject), &|_| {
        false
    });
    let lines = found
        .iter()
        .map(|call| match &call.callee {
            Callee::Function(name) => format!("{}()", hir.name_ref(*name).path),
            Callee::Member { name, .. } => format!("{name}()"),
            Callee::Constructed(_) => "new".to_owned(),
        })
        .collect::<Vec<_>>();
    match lines.is_empty() {
        true => "<no calls>".to_owned(),
        false => lines.join("\n"),
    }
}

/// Three calls took an integer -- two literals and a cast -- and none of them took *this* one.
#[test]
fn an_integer_is_not_the_integer_another_call_was_handed() {
    let answer = handed_to(|_| ValuePath {
        root: PathRoot::Builtin(BuiltinTy::Int),
        steps: Vec::new(),
        may_be_absent: false,
    });
    expect!["<no calls>"].assert_eq(&answer);
}

/// The control, on the same body: a local is a name, and the call it was handed to is found.
#[test]
fn a_local_is_still_found_where_it_was_handed_over() {
    let answer = handed_to(|interner| local_path(&interner.name("value")));
    expect!["takes()"].assert_eq(&answer);
}
