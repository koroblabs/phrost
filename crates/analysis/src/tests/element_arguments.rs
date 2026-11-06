//! An array argument whose *element* type the declaration disagrees with.
//!
//! `Row[]` given a `string[]` is the commonest real bug an array can carry in PHP, and until both
//! sides knew their elements it could not be judged: the compatibility rules dropped every type
//! argument, deliberately and with the doc comments saying so, because reading one side alone invents
//! findings nothing can corroborate.
//!
//! Now both sides say. A parameter states its element in a docblock; a value's comes from an
//! annotation, an inferred container, or a list literal. So the rule is elementwise -- and everything
//! below is about the cases where it must still say nothing.
//!
//! # What it does not touch
//!
//! Only when **both** sides know. An array of unknown elements is an array and answers as it did; so
//! is a shape, and so is `array<string, mixed>`, which is 290 of the 526 element-typed parameters on
//! the reference project.
//!
//! Covariant, and for PHP that is sound rather than lenient: an array is passed by value, so a callee
//! writing into it cannot reach the caller's copy.
//!
//! And the coercions are the scalar ones, because the comparison *is* the scalar comparison one level
//! in: `int[]` satisfies `float[]` exactly as an `int` satisfies a `float`, and a `class-string[]`
//! satisfies `string[]`.
//!
//! # A literal is judged slot by slot
//!
//! `[1, $unknown]` against a `Row[]` was silent, because a literal's type was the *union* of what
//! its entries held and a union built from the ones that answered is narrower than the truth. A
//! positional literal now carries its slots -- `array{0: int, 1: mixed}` -- and each is judged on
//! its own, so the entry that is definitely wrong is reported whatever the one beside it holds.
//! The approximation the all-or-nothing rule guarded against is gone, so the rule is too.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_element_the_declaration_disagrees_with_is_reported_and_nothing_else_is() {
    check(
        r#"
//- /app.php
<?php

declare(strict_types=1);

class Row {}
class SubRow extends Row {}
interface RowLike {}

class Needs
{
    /** @param Row[] $rows */
    public function rows(array $rows): void {}

    /** @param string[] $names */
    public function names(array $names): void {}

    /** @param float[] $numbers */
    public function numbers(array $numbers): void {}

    /** @param RowLike[] $like */
    public function like(array $like): void {}

    /**
     * @param string[] $strings
     * @param int[] $ints
     * @param SubRow[] $subs
     * @param Row[] $rows
     * @param array<string, mixed> $anyKeyed
     */
    public function run(
        Needs $n,
        array $strings,
        array $ints,
        array $subs,
        array $rows,
        array $anyKeyed,
        array $bare,
        mixed $unknown
    ): void {
        $n->rows($strings);
        $n->names($ints);
        $n->rows(['a']);
        $n->names([1, 'x']);

        $n->rows($subs);
        $n->rows($rows);
        $n->numbers($ints);
        $n->like($rows);
        $n->rows($anyKeyed);
        $n->rows($bare);
        $n->names([Row::class]);
        $n->rows([]);
        $n->rows([1, $unknown]);
        $n->rows(['a' => new Row()]);
    }
}
"#,
        expect![[r#"
            40:18-40:26 error [incompatible-argument] $rows takes \Row[], and this is string[]
            41:19-41:24 error [incompatible-argument] $names takes string[], and this is int[]
            42:18-42:23 error [incompatible-argument] $rows takes \Row[], and this is array{0: non-falsy-string}
            43:19-43:27 error [incompatible-argument] $names takes string[], and this is array{0: int, 1: non-falsy-string}
            48:18-48:23 error [incompatible-argument] $like takes \RowLike[], and this is Row[]
            53:18-53:31 error [incompatible-argument] $rows takes \Row[], and this is array{0: int, 1: mixed}
        "#]],
    );
}
