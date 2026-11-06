//! An enum reported for not declaring what PHP declares for it.
//!
//! `cases()` exists on every enum, and `from()` and `tryFrom()` on every backed one, without being
//! written anywhere. The member table absorbs them from the real `UnitEnum` -- rather than
//! synthesizing them, so `$stage->value` navigates somewhere that exists -- and they arrive with the
//! provenance of an interface requirement, which is what a check for unimplemented methods looks for.
//!
//! That was already handled for the ordinary case, by requiring a supertype to be *written* before it
//! can require anything: PHP forbids `enum X implements UnitEnum` outright, so nothing wrote it.
//!
//! What was not handled is that an interface may **extend** it. `interface HasCases extends UnitEnum`
//! with `enum Stage implements HasCases` is valid PHP, and it puts `UnitEnum` into the written set --
//! so every enum written that way was reported for not declaring `cases()`. The vendored standard
//! library carries two of its own (`RoundingMode`, `SortDirection`).
//!
//! The fixture declares `UnitEnum` itself, because without it the member table is *incomplete* and the
//! check declines for that reason instead -- a test that reads as passing and proves nothing.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_enum_reached_through_an_interface_extending_unitenum_is_not_missing_cases() {
    check(
        r#"
//- /src/Php.php
<?php

interface UnitEnum
{
    public static function cases(): array;
}

interface BackedEnum extends UnitEnum
{
    public static function from(string|int $value): static;

    public static function tryFrom(string|int $value): ?static;
}

//- /src/Stages.php
<?php
namespace App;

interface HasCases extends \UnitEnum
{
}

interface Labelled
{
    public function label(): string;
}

enum Stage implements HasCases
{
    case Draft;
}

enum Shipped: string implements HasCases, Labelled
{
    case Yes = 'y';

    public function label(): string
    {
        return 'yes';
    }
}

enum Unlabelled: string implements HasCases, Labelled
{
    case No = 'n';
}
"#,
        expect![[r#"
            28:6-28:16 error [unimplemented-method] App\Unlabelled does not implement App\Labelled::label()
        "#]],
    );
}
