//! Every write into a local's slots after the literal that built it -- see `filled_slots`.
//!
//! Rendered one per line: where, which slot, what went in, and whether it certainly ran. A write the
//! literal comes *after* is not one of them, and neither is one the position cannot have seen.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner, TextSize};

use crate::{FilledSlot, local_binding, slots_filled_since};

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
    assert!(lowered.errors.is_empty(), "fixture does not parse");
    let hir = &lowered.hir;
    let since = local_binding(hir, offset, variable, &|_, _| false, &crate::NoAssertions)
        .and_then(|bound| bound.written_at)
        .expect("the variable is assigned here");
    let fills = slots_filled_since(hir, offset, variable, since, &|_, _| false);
    let line_of = |at: TextSize| text[..usize::from(at)].matches('\n').count() + 1;
    let rendered = fills
        .iter()
        .map(|fill| {
            let slot = match &fill.slot {
                FilledSlot::Appended => "[]".to_owned(),
                FilledSlot::Named(key) => format!("[{key}]"),
                FilledSlot::Computed => "[?]".to_owned(),
                FilledSlot::Through(Some(key)) => format!("[{key}][..]"),
                FilledSlot::Through(None) => "[?][..]".to_owned(),
                FilledSlot::Removed(Some(key)) => format!("unset [{key}]"),
                FilledSlot::Removed(None) => "unset [?]".to_owned(),
            };
            let value = match (&fill.value, fill.compound) {
                (_, true) => "compound".to_owned(),
                (Some(value), false) => super::render(hir, value),
                (None, false) => "<unread>".to_owned(),
            };
            let certain = if fill.certain { "certain" } else { "may have" };
            format!("line {}: {slot} = {value}, {certain}\n", line_of(fill.at))
        })
        .collect::<String>();
    expect.assert_eq(&rendered);
}

#[test]
fn the_writes_after_the_literal_in_source_order() {
    check(
        r#"<?php
function run(bool $flag, string $path, int $n, string $key): void
{
    $args = ['--force'];
    $args[] = $path;
    $args['mode'] = $n;
    $args[$key] = 1;
    $args['nested'][] = 2;
    $args['count'] += 1;
    if ($flag) {
        $args[] = $flag;
    }
    $a$0rgs;
    $args[] = 'after';
}
"#,
        "args",
        expect![[r#"
            line 5: [] = $path, certain
            line 6: [mode] = $n, certain
            line 7: [?] = (int), certain
            line 8: [nested][..] = (int), certain
            line 9: [count] = compound, certain
            line 11: [] = $flag, may have
        "#]],
    );
}

#[test]
fn a_loop_carries_its_writes_round_to_a_literal_written_before_it() {
    check(
        r#"<?php
function run(array $children): void
{
    $stack = ['root'];
    foreach ($children as $child) {
        $st$0ack;
        $stack[] = $child;
    }
}
"#,
        "stack",
        expect![[r#"
            line 7: [] = $child, may have
        "#]],
    );
}

#[test]
fn and_not_to_one_written_afresh_on_every_pass() {
    check(
        r#"<?php
function run(array $children): void
{
    foreach ($children as $child) {
        $row = ['id' => 1];
        $r$0ow;
        $row['child'] = $child;
    }
}
"#,
        "row",
        expect![""],
    );
}

/// An `unset` takes a slot out, and is found the way a write is: in order, and only where it can
/// have run before the position.
#[test]
fn an_unset_is_a_removal_among_the_writes() {
    check(
        r#"<?php
function run(bool $flag, string $key): void
{
    $row = ['id' => 1, 'tmp' => 2, 'meta' => ['a' => 1]];
    unset($row['tmp']);
    if ($flag) {
        unset($row['id']);
    }
    unset($row[$key], $other['id']);
    unset($row['meta']['a']);
    $r$0ow;
    unset($row['after']);
}
"#,
        "row",
        expect![[r#"
            line 5: unset [tmp] = <unread>, certain
            line 7: unset [id] = <unread>, may have
            line 9: unset [?] = <unread>, certain
            line 10: [meta][..] = <unread>, certain
        "#]],
    );
}
