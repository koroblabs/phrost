//! A closure handed where a documented signature is expected, declaring a parameter that cannot
//! take what the signature says it will be handed.
//!
//! PHP checks a closure's parameters when it is *called*, so the question is how the callee calls
//! it. An object against a scalar, or two classes that refuse each other, fail in either mode. Two
//! scalar kinds fail only where the callee's own file is strict: an internal function, and a
//! function in a file without `strict_types`, convert the value and the call runs.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_parameter_the_signature_cannot_hand_a_value_to_is_reported() {
    check(
        r#"
//- /src/Run.php
<?php
declare(strict_types=1);

namespace App;

final class Order {}
final class Invoice {}

/** @param callable(int): string $format */
function format(callable $format): void {}

/** @param callable(Order): void $visit */
function visit(callable $visit): void {}

function go(): void
{
    format(static fn (int $value): string => (string) $value);
    format(static fn (string $value): string => $value);
    format(static fn (float $value): string => (string) $value);
    format(static fn ($value): string => (string) $value);
    format(static fn (Order $value): string => '');
    visit(static function (Order $order): void {});
    visit(static function (Invoice $invoice): void {});
    visit(static function (int $id): void {});
}
"#,
        expect![[r#"
            18:12-18:55 error [incompatible-argument] $format takes callable(int): string, and this is Closure(string $value): string
            21:12-21:50 error [incompatible-argument] $format takes callable(int): string, and this is Closure(Order $value): string
            23:11-23:54 error [incompatible-argument] $visit takes callable(\App\Order): void, and this is Closure(Invoice $invoice): void
            24:11-24:45 error [incompatible-argument] $visit takes callable(\App\Order): void, and this is Closure(int $id): void
        "#]],
    );
}

#[test]
fn a_callee_that_converts_scalars_is_not_refused_one() {
    // The same closure, handed to a function declared in a file without `strict_types`: it converts
    // the `int` to a string and the call runs. The class beside it is refused in every mode.
    check(
        r#"
//- /src/Format.php
<?php
namespace App;

final class Order {}

/** @param callable(int): string $format */
function format(callable $format): void {}

//- /src/Run.php
<?php
declare(strict_types=1);

namespace App;

function go(): void
{
    format(static fn (string $value): string => $value);
    format(static fn (Order $value): string => '');
}
"#,
        expect![[r#"
            9:12-9:50 error [incompatible-argument] $format takes callable(int): string, and this is Closure(Order $value): string
        "#]],
    );
}
