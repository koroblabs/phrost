//! A `mixed` by-reference argument invalidates what the caller knew, and the check must see that.
//!
//! `assertSeverityLevel(mixed &$level)` narrows its argument internally, so a caller holding
//! `Level|int` is holding a stale claim by the time the call has returned -- and assigning it to a
//! `Level` property was reported on one real project. PHPStan says nothing there.
//!
//! Keeping the caller's own type is not the cautious reading of `mixed &$x`; it is the unsound one,
//! and it is unsound in the direction that *reports*. Widening to `mixed` can only ever remove a
//! finding, because every check treats `mixed` as no claim -- which is why this needs no reading of
//! the callee's body to be right.
//!
//! The union case is deliberately different and stays reported: a union by-reference parameter is an
//! upper bound on what the callee *accepts*, and believing it widened `array_walk($rows)` from
//! `array` to `object|array` and reported every later `array_shift`. Making the union case silent
//! needs `@param-out`, which the bundled stubs write 145 times and which nothing reads yet.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_mixed_reference_is_no_claim_and_a_union_reference_is_still_the_callers_own() {
    check(
        r#"
//- /app.php
<?php
enum Level: int
{
    case Low = 1;
}

class Holder
{
    private Level $level;

    /// The body is not read and does not need to be: what makes the caller's claim stale is the
    /// parameter being by-reference and `mixed`, not what happens in here.
    private function normalise(mixed &$level): void
    {
        $level = Level::Low;
    }

    private function unioned(int|Level &$level): void
    {
        $level = Level::Low;
    }

    public function viaMixed(Level|int $a): void
    {
        $this->normalise($a);
        $this->level = $a;
    }

    public function viaUnion(Level|int $b): void
    {
        $this->unioned($b);
        $this->level = $b;
    }
}
"#,
        expect![[r#"
            32:24-32:26 error [incompatible-assignment] Holder::$level takes \Level, and this is Level|int
        "#]],
    );
}
