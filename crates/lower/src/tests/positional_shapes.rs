//! `array{Thing, string}` -- a shape whose entries name no key.
//!
//! PHP gives a keyless entry the next free integer key, so `array{int, string}` *is*
//! `array{0: int, 1: string}`. Lowering used to drop those entries, with the stated reason that
//! nothing reads a shape by position -- which was circular, since nothing read a position because
//! nothing stored one, and `$row[0]` on a shape keyed `0` resolved the whole time. Dropping them
//! took the shape with it: a shape with no readable field falls back to a bare `array`, so
//! `array{Thing, string}` said nothing at all.
//!
//! The fixtures declare no native return hint on purpose: `check_hir` renders a docblock's return
//! only where the signature carries none, so a `: array` beside these would hide the very field the
//! test is about. Third time in this session that a snapshot which does not show a field could not
//! test it.
//!
//! At this layer because that is where the key is invented, and because the numbering rule is what
//! the test is about: an explicit integer key advances the counter exactly as it does in an array
//! literal, which is the one part a reader could get wrong without any snapshot noticing.

use expect_test::expect;

use super::check_hir;

#[test]
fn a_keyless_entry_takes_the_next_free_integer_key() {
    check_hir(
        r#"
//- /src/app.php
<?php

/** @return array{Thing, string} */
function tuple() {}

/** @return array<array{Thing, string}> */
function nested() {}

/** A keyed shape is untouched, and so is a numeric one written out. */
/** @return array{first: Thing, second: string} */
function keyed() {}

/** @return array{0: Thing, 1: string} */
function numbered() {}
"#,
        expect![[r#"
            function tuple() doc: array{0: Thing, 1: string} @ 4:10-4:15  [doc]
            function nested() doc: array<array{0: Thing, 1: string}> @ 7:10-7:16  [doc]
            function keyed() doc: array{first: Thing, second: string} @ 11:10-11:15  [doc]
            function numbered() doc: array{0: Thing, 1: string} @ 14:10-14:18  [doc]
        "#]],
    );
}

#[test]
fn an_explicit_integer_key_advances_the_counter() {
    check_hir(
        r#"
//- /src/app.php
<?php

/** The rule PHP's array literals follow: `B` is 1, not 0. */
/** @return array{0: Thing, string} */
function after() {}

/** A named key claims no integer, so the entry after it is still 1. */
/** @return array{Thing, named: string, Other} */
function mixed() {}

/** A key out of order takes the counter with it. */
/** @return array{5: Thing, string} */
function jumped() {}

/** An optional entry is one only a written key can be, so a keyless one never is. */
/** @return array{Thing, 1?: string} */
function optional() {}
"#,
        expect![[r#"
            function after() doc: array{0: Thing, 1: string} @ 5:10-5:15  [doc]
            function mixed() doc: array{0: Thing, named: string, 1: Other} @ 9:10-9:15  [doc]
            function jumped() doc: array{5: Thing, 6: string} @ 13:10-13:16  [doc]
            function optional() doc: array{0: Thing, 1?: string} @ 17:10-17:18  [doc]
        "#]],
    );
}
