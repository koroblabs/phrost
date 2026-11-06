//! What PHP refuses about a parameter list, wherever one is written.
//!
//! A function, a method, a closure and an arrow function all carry one, and PHP refuses all four the
//! same way -- so the rules are about the parameters beside each other and nothing else, which is
//! what lets one walk answer for every place a list can appear.
//!
//! Each was run against PHP 8.5.10, and three of the answers are not what the wording suggests: a
//! variable is **case-sensitive**, so `f($a, $A)` compiles and only an exact repeat is the fatal; a
//! **by-reference** promoted property is legal and a **variadic** one is not; and a promoted
//! property in an abstract or interface constructor is refused with a different sentence from one
//! outside a constructor, because the two are different mistakes.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn every_list_php_will_not_compile_is_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

function twice($a, $a): void {}

function claims($this): void {}

function early(...$rest, $after): void {}

function defaulted(...$rest = []): void {}

class Promotes
{
    public function notAConstructor(public int $x): void {}

    public function __construct(public int ...$many) {}
}

abstract class Bodiless
{
    abstract public function __construct(public int $x);
}

function inner(): void
{
    $closure = function ($b, $b): void {};
    $arrow = fn ($c, $c): int => 1;
}
"#,
        expect![[r#"
            4:20-4:22 error [refused-declaration] $a is declared twice in this parameter list
            6:17-6:22 error [refused-declaration] $this cannot be a parameter -- it is the receiver
            8:19-8:24 error [refused-declaration] ...$rest takes every argument after it, so nothing may follow it
            10:23-10:28 error [refused-declaration] ...$rest takes every argument after it, so a default says nothing
            14:48-14:50 error [refused-declaration] $x promotes a property, which only a constructor may do
            16:47-16:52 error [refused-declaration] ...$many is variadic, and a variadic cannot promote a property
            21:53-21:55 error [refused-declaration] $x promotes a property, and this constructor has no body to assign it in
            26:5-26:13 warning [unused-variable] $closure is assigned and never read
            26:30-26:32 error [refused-declaration] $b is declared twice in this parameter list
            27:5-27:11 warning [unused-variable] $arrow is assigned and never read
            27:22-27:24 error [refused-declaration] $c is declared twice in this parameter list
        "#]],
    );
}

/// And every shape PHP accepts, which must stay silent. Two of them look like the rules above and
/// are not: `$A` beside `$a` is a second variable, and a by-reference promoted property compiles.
#[test]
fn every_list_php_compiles_is_left_alone() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

function cased($a, $A): void {}

function named($This, $THIS): void {}

function trailing($first, ...$rest): void {}

class Promotes
{
    public function __construct(
        public readonly int $kept,
        protected string $also,
        private array &$byReference,
    ) {}
}

function inner(): void
{
    $closure = function ($b, ...$rest): void {};
    $arrow = fn ($c): int => 1;
    $closure(1);
    $arrow(2);
}
"#,
        expect![[r#""#]],
    );
}
