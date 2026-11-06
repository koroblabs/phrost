//! An object of a class nothing can extend, where a string, a callable or an iterable is wanted.
//!
//! Whether an object can stand in for one of those is a question about what its class declares --
//! `__toString`, `__invoke`, `Traversable` -- and a class lacking it today is one a subclass may
//! still supply, so the judgement stays silent. A `final` class and an enum have no subclass, and
//! what they lack they lack: `takes(string $s)` handed one is the `TypeError` PHP raises.
//!
//! An enum is the other half. PHP gives every enum `UnitEnum` and every backed one `BackedEnum`
//! without either being written, and the hierarchy did not list them -- so `\UnitEnum $case` refused
//! every enum there is. Declared here as the standard library declares them, since a fixture does
//! not index the stubs.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_closed_class_is_not_what_it_does_not_declare() {
    check(
        r#"
//- /src/app.php
<?php

final class Plain
{
}

final class Printable
{
    public function __toString(): string
    {
        return '';
    }
}

class Open
{
}

function takes_string(string $s): void
{
}

function takes_callable(callable $c): void
{
}

function takes_iterable(iterable $i): void
{
}

function run(Plain $plain, Printable $printable, Open $open): void
{
    takes_string($plain);
    takes_string($printable);
    takes_string($open);
    takes_callable($plain);
    takes_iterable($plain);
}
"#,
        expect![[r#"
            33:18-33:24 error [incompatible-argument] $s takes string, and this is Plain
            36:20-36:26 error [incompatible-argument] $c takes callable, and this is Plain
            37:20-37:26 error [incompatible-argument] $i takes iterable, and this is Plain
        "#]],
    );
}

#[test]
fn every_enum_is_a_unit_enum_and_a_backed_one_a_backed_enum() {
    check(
        r#"
//- /src/app.php
<?php

interface UnitEnum
{
}

interface BackedEnum extends UnitEnum
{
}

enum Pure
{
    case A;
}

enum Backed: string
{
    case A = 'a';
}

function takes_unit(UnitEnum $case): void
{
}

function takes_backed(BackedEnum $case): void
{
}

function takes_key(UnitEnum|string $key): void
{
}

function takes_string(string $s): void
{
}

function run(): void
{
    takes_unit(Pure::A);
    takes_unit(Backed::A);
    takes_backed(Backed::A);
    takes_key(Backed::A);
    takes_backed(Pure::A);
    takes_string(Backed::A);
}
"#,
        expect![[r#"
            43:18-43:25 error [incompatible-argument] $case takes \BackedEnum, and this is Pure
            44:18-44:27 error [incompatible-argument] $s takes string, and this is Backed
        "#]],
    );
}
