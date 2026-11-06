//! A value passed on after a call that said what it leaves in its receiver is judged as what it
//! became.
//!
//! The third-party conformance test `assertions_this_out_self_out`, reduced: a box of `int` given a
//! `string` through `@phpstan-self-out self<U>` is a box of `string` afterwards, so handing it to a
//! function wanting one is fine and handing it to one wanting the box of `int` is not -- the second
//! report is the one that proves the first silence is the narrowing and not the check going quiet.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn after_the_call_the_box_is_judged_as_what_it_was_given() {
    check(
        r#"
//- /app.php
<?php
/** @template T */
final class MutableBox
{
    /** @param T $value */
    public function __construct(public mixed $value) {}

    /**
     * @template U
     * @param U $value
     * @phpstan-self-out self<U>
     */
    public function replace(mixed $value): void {}
}

/** @param MutableBox<int> $box */
function takesIntBox(MutableBox $box): void {}

/** @param MutableBox<string> $box */
function takesStringBox(MutableBox $box): void {}

function inspect(string $next): void
{
    $box = new MutableBox(1);
    takesStringBox($box);
    $box->replace($next);
    takesStringBox($box);
    takesIntBox($box);
}
"#,
        expect![[r#"
            25:20-25:24 error [incompatible-argument] $box takes \MutableBox<string>, and this is MutableBox<int>
            28:17-28:21 error [incompatible-argument] $box takes \MutableBox<int>, and this is MutableBox<string>
        "#]],
    );
}
