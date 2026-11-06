//! A `return` or a `yield` the declared return type will not take.
//!
//! Four compile-time fatals, and three of them needed a corpus to get the boundary right.
//!
//! * A **generator**'s `return` is not the function's: a bare one ends the iteration and a value is
//!   what `getReturn()` hands back. Symfony writes 56 of those and psl 23.
//! * An **arrow function**'s `return` is synthesized by lowering rather than written, so
//!   `fn (): never => throw $t` is not a return at all -- psl writes it sixteen times. `fn (): void
//!   => 1` still is, because an arrow function's body always has a value.
//! * A `yield` is refused only by `void`, `never` and the scalars. `object` and `mixed` accept one,
//!   because PHP asks that the declared type be a *supertype* of `Generator` -- `array` is the one
//!   in the refused list a reader expects to work.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn what_the_declared_type_will_not_take() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Returns
{
    public function voidWithValue(): void
    {
        return 1;
    }

    public function typedWithNothing(): int
    {
        return;
    }

    public function neverReturns(): never
    {
        return;
    }

    public function voidYields(): void
    {
        yield 1;
    }

    public function arrayYields(): array
    {
        yield 1;
    }
}
"#,
        expect![[r#"
            8:9-8:18 error [refused-return] a void function must not return a value
            13:9-13:16 error [refused-return] this declares a return type, so `return` needs a value
            18:9-18:16 error [refused-return] a never-returning function must not return at all
            22:5-24:6 error [refused-return] this yields, and the declared return type cannot be a generator
            27:5-29:6 error [refused-return] this yields, and the declared return type cannot be a generator
        "#]],
    );
}

/// What PHP takes, which is most of the shapes that look wrong.
#[test]
fn a_generator_an_arrow_and_a_wide_type_are_left_alone() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Failure {}

class Returns
{
    public function generatorEndsEarly(): iterable
    {
        yield 1;

        return;
    }

    public function generatorHandsBack(): iterable
    {
        yield 1;

        return 5;
    }

    public function objectYields(): object
    {
        yield 1;
    }

    public function mixedYields(): mixed
    {
        yield 1;
    }

    public function bareVoid(): void
    {
        return;
    }

    public function arrows(): callable
    {
        return static fn (Failure $t): never => throw $t;
    }

    public function untyped()
    {
        return;
    }
}
"#,
        expect![""],
    );
}
