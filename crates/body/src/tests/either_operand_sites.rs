//! Where each arm of a join is written, which is where its names mean what they mean.
//!
//! The other files about `??`, the ternary and `match` pin *which* operands a join keeps. This one
//! pins the position each carries, because that is the whole of what a reader needs to resolve an
//! arm as the code selected it: `is_array($x) ? $x[1] : $x` picks each side by a condition, and an
//! arm read at the join is the value the condition was written to split.
//!
//! Its own renderer rather than the shared one, which prints paths and not positions. A position is
//! invisible in a path rendering, so a test written against that harness would pass whichever
//! position the operands carried -- which is the one thing this file exists to say.

use expect_test::{Expect, expect};
use phrost_hir::FileHir;
use phrost_model::{FileId, Interner, TextSize};

use crate::{PathRoot, local_binding};

/// Every operand of the join `variable` holds at the `$0` marker, with where it is written.
///
/// `line:column`, one-based as an editor counts, so a snapshot reads against the fixture beside it.
fn check(source: &str, variable: &str, expect: Expect) {
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
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );

    let rendered = match local_binding(
        &lowered.hir,
        offset,
        variable,
        &|_, _| false,
        &crate::NoAssertions,
    ) {
        Some(bound) => match &bound.value.root {
            PathRoot::EitherOf { operands, .. } => operands
                .iter()
                .map(|operand| {
                    format!(
                        "{} at {}\n",
                        rendered_operand(&lowered.hir, &operand.value),
                        line_column(&text, operand.at)
                    )
                })
                .collect(),
            _ => "<not a join>\n".to_owned(),
        },
        None => "<not declared here>\n".to_owned(),
    };
    expect.assert_eq(&rendered);
}

/// The operand's path, borrowed from the shared renderer so this file states positions and nothing
/// else.
fn rendered_operand(hir: &FileHir, path: &crate::ValuePath) -> String {
    super::render(hir, path)
}

/// One-based `line:column`, counting characters the way an editor's status bar does.
fn line_column(text: &str, offset: TextSize) -> String {
    let before = &text[..usize::from(offset)];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit_once('\n')
        .map_or(before, |(_, last)| last)
        .chars()
        .count()
        + 1;
    format!("{line}:{column}")
}

#[test]
fn a_ternarys_arms_are_written_where_each_arm_begins() {
    // The position is each arm's own start, not the join's and not the arm's end. Its start is the
    // first place the arm is being evaluated, so the condition that chose it is already in effect
    // and nothing the arm itself writes has happened yet.
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
        expect![[r#"
            new Foo at 5:15
            new Bar at 5:27
        "#]],
    );
}

#[test]
fn a_coalesces_operands_are_written_where_each_operand_begins() {
    // `??` reaches its right operand only where the left was null, which is a narrowing nothing
    // applies yet -- but the position it would need is the one recorded here, and it is the right
    // operand's own start for the same reason a ternary arm's is.
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
        expect![[r#"
            maybeWorker() at 7:10
            new Fallback at 7:27
        "#]],
    );
}

#[test]
fn a_short_ternary_writes_its_left_operand_where_the_condition_begins() {
    // `?:` has no then-branch: the value it produces when the condition was truthy *is* the
    // condition, so the operand and the position both come from there.
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
        expect![[r#"
            maybeWorker() at 7:10
            new Fallback at 7:27
        "#]],
    );
}

#[test]
fn a_match_writes_each_arm_where_its_body_begins() {
    // The arm's body and not the arm: the conditions contribute nothing to the value, so the
    // position that means anything for the value is where the body starts. A `throw` arm is not an
    // operand and so has no position here either.
    check(
        r#"
<?php
function run(int $k): void
{
    $x = match ($k) {
        1 => new Foo(),
        2, 3 => new Bar(),
        default => throw new RuntimeException(),
    };
    $x;
    $0
}
"#,
        "x",
        expect![[r#"
            new Foo at 6:14
            new Bar at 7:17
        "#]],
    );
}

#[test]
fn a_nested_join_carries_positions_at_every_level() {
    // An arm that is itself a join keeps its own arms' positions, which is what lets the inner
    // condition narrow the inner arms independently of the outer one.
    check(
        r#"
<?php
function run(bool $c, bool $d): void
{
    $x = $c ? new Foo() : ($d ? new Bar() : new Baz());
    $x;
    $0
}
"#,
        "x",
        expect![[r#"
            new Foo at 5:15
            new Bar or new Baz at 5:28
        "#]],
    );
}
