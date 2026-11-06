//! A write a `break` leaves behind, asked from inside the loop and from after it.
//!
//! `Leaving` models what leaves the **function** -- `return`, `throw`, `exit` -- because that is
//! what decides whether a write in a branch can reach a position after it. A `break` leaves the
//! *loop*, which is a different question with two answers: it leaves past every position still in
//! that loop, and it reaches every position after it.
//!
//! So the branch carries both, derived once, and the reader picks by where the position is. Without
//! the second, `$entry = FALSE; break;` joined a `false` into every read below it in the same loop
//! -- a value control can never bring there.
//!
//! The machinery was already written: `Leaving::of` has taken a `break_escapes` flag since switch
//! arms needed one. What was missing is a branch keeping both answers, and the one bit of position
//! that chooses between them.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner, TextSize};

/// Renders what the local at `$0` holds.
fn check(source: &str, expect: Expect) {
    let at = source.find("$0").expect("the source marks $0");
    let text = source.replacen("$0", "", 1);
    let offset = TextSize::try_from(at).expect("fixture fits in u32");

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

    let lines = phrost_model::LineIndex::new(&text);
    let binding = crate::local_binding(
        &lowered.hir,
        offset,
        "entry",
        &|_, _| false,
        &crate::NoAssertions,
    );
    let out = match binding {
        Some(bound) => {
            let at = lines.line_col(bound.written_at.unwrap_or(offset));
            format!("bound at line {}\n", at.line + 1)
        }
        None => "nothing binds it\n".to_owned(),
    };
    expect.assert_eq(&out);
}

#[test]
fn a_write_the_loop_breaks_out_of_does_not_reach_a_read_below_it() {
    check(
        r#"<?php
function walk(array $data, array $parts): void
{
    $entry = $data;
    foreach ($parts as $part) {
        if ($part) {
            $entry = FALSE;
            break;
        }
        echo $en$0try;
    }
}
"#,
        expect![[r#"
            bound at line 4
        "#]],
    );
}

/// The same rule the other way round: a write the loop leaves has no pass after it, so nothing it
/// wrote comes round to a read **above** it either.
///
/// This is what the back edge had to learn second. `if (!isset($entry)) { $entry = FALSE; break; }`
/// carried the `false` up to the `isset` on the line above -- and from there the guard handed it
/// back, since ruling out null leaves a `false` standing. Isolated by bisecting three axes at once:
/// a loop, an `isset`, and a branch that escapes. Only all three together produced it, which is why
/// six earlier reproductions of "the same shape" were silent.
#[test]
fn a_write_the_loop_leaves_is_carried_to_nothing_above_it() {
    check(
        r#"<?php
function walk(?array $data, bool $flag): void
{
    $entry = $data;
    while ($flag) {
        if (!isset($en$0try)) {
            $entry = FALSE;
            break;
        }
    }
}
"#,
        expect![[r#"
            bound at line 4
        "#]],
    );
}

/// The other half, and what keeps the first honest: a position **after** the loop is exactly where
/// a `break` sends control, so the write reaches it and the two are joined.
#[test]
fn the_same_write_reaches_a_read_after_the_loop() {
    check(
        r#"<?php
function walk(array $data, array $parts): void
{
    $entry = $data;
    foreach ($parts as $part) {
        if ($part) {
            $entry = FALSE;
            break;
        }
    }
    echo $en$0try;
}
"#,
        expect![[r#"
            bound at line 7
        "#]],
    );
}
