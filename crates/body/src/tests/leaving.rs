//! Whether a body always leaves before falling off its end.
//!
//! What lets a guard narrow the code *after* it. Conservative in one direction only: the answer is
//! either "yes, certainly" or "no", and an unmodelled shape is a "no".

use phrost_model::{FileId, Interner};

/// Lowers `source`, which must declare exactly one free function, and asks whether its body
/// always leaves before falling off the end.
///
/// `always_leaves` is `pub(crate)`, tested directly here rather than only through a feature built
/// on it: a `switch` is not an `if`, and nothing else in this crate exercises it on one yet.
fn leaves(source: &str) -> bool {
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        source,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );

    let hir = &lowered.hir;
    let body = hir
        .functions()
        .next()
        .expect("fixture declares one function")
        .1
        .body
        .expect("bodies were lowered");

    // No index here, so nothing is known to return `never`: these tests are about the shapes
    // syntax settles on its own.
    crate::walk::always_leaves(hir, hir.body(body).statements, &|_, _| false)
}

#[test]
fn a_switch_leaves_when_it_has_a_default_and_every_arm_returns() {
    assert!(leaves(
        r#"<?php
function run($value): string
{
    switch ($value) {
        case 1:
            return 'one';
        case 2:
            return 'two';
        default:
            return 'other';
    }
}
"#
    ));
}

#[test]
fn a_switch_without_a_default_never_leaves() {
    // Every present arm returns, but a value matching none of them falls through the whole
    // statement -- there being no arm at all is indistinguishable from one that does not leave.
    assert!(!leaves(
        r#"<?php
function run($value): string
{
    switch ($value) {
        case 1:
            return 'one';
        case 2:
            return 'two';
    }
}
"#
    ));
}

#[test]
fn a_switch_with_an_arm_that_falls_off_the_end_does_not_leave() {
    // A default exists, but `case 1` neither returns nor breaks -- falling past the switch is
    // still possible, so the statement as a whole is only "maybe".
    assert!(!leaves(
        r#"<?php
function run($value): string
{
    switch ($value) {
        case 1:
            $unused = $value;
        default:
            return 'other';
    }
}
"#
    ));
}

#[test]
fn a_switch_with_folded_fallthrough_labels_still_leaves() {
    // `case 1: case 2: return 'x';` is one arm (see `SwitchCaseData`) whose single body leaves --
    // not two arms, one of which would wrongly look like it does not.
    assert!(leaves(
        r#"<?php
function run($value): string
{
    switch ($value) {
        case 1:
        case 2:
            return 'small';
        default:
            return 'other';
    }
}
"#
    ));
}

#[test]
fn a_switch_whose_arms_only_break_leaves_nothing() {
    // A `break` in an arm is consumed by the switch: execution continues on the line after it. A
    // narrowing that treated this as leaving would claim something about code plainly reachable.
    assert!(!leaves(
        r#"<?php
function run($value): void
{
    switch ($value) {
        case 1:
            break;
        default:
            break;
    }
}
"#
    ));
}

#[test]
fn a_break_still_leaves_the_iteration_it_is_written_in() {
    // The reason `break` counts at all: after a guard that breaks, the rest of that iteration only
    // runs when the condition held.
    assert!(leaves(
        r#"<?php
function run($value): void
{
    break;
}
"#
    ));
}
