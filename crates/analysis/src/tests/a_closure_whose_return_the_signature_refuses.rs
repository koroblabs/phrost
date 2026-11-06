//! A closure handed where a documented signature is expected, declaring a return the signature's
//! cannot be.
//!
//! Only the declared return, and only when the two share no value -- `int` for `string`, a scalar
//! for an object, two classes that refuse each other. An arrow function with no declared return,
//! and one whose return the signature's may hold, say nothing.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn only_a_declared_return_the_signature_cannot_hold_is_reported() {
    check(
        r#"
//- /stubs/Closure.php
<?php
final class Closure {}

//- /src/Run.php
<?php
namespace App;

/** @param callable(int): string $format */
function format(callable $format): void {}

/** @param \Closure(int): bool $keep */
function keep(\Closure $keep): void {}

function go(): void
{
    format(static fn (int $value): string => (string) $value);
    format(static fn (int $value): int => $value);
    format(static fn (int $value) => $value);
    keep(static fn (int $value): bool => $value > 1);
    keep(static fn (int $value): int => $value);
    $named = static fn (int $value): int => $value;
    format($named);
}
"#,
        expect![[r#"
            13:12-13:49 error [incompatible-argument] $format takes callable(int): string, and this is Closure(int $value): int
            16:10-16:47 error [incompatible-argument] $keep takes callable(int): bool, and this is Closure(int $value): int
            18:12-18:18 error [incompatible-argument] $format takes callable(int): string, and this is Closure(int $value): int
        "#]],
    );
}
