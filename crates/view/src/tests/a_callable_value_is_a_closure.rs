//! `strlen(...)` and `function () {}` are `Closure` objects, and PHP's class of that name is where
//! their members live.
//!
//! The callable arm answered no class, with the reason that naming `Closure` would offer its
//! members on a value written as a plain `callable` -- which may be a string or a two-element array
//! instead. True of the *declaration* and not of this arm: `Held::Callable` is built in exactly two
//! places, a closure literal and a first-class callable, and PHP makes a `Closure` out of both. A
//! plain `callable` parameter arrives as a written or stored type whose builtin names no class,
//! which is the answer it should have and is untouched.
//!
//! So `$fn = strlen(...); $fn->bindTo(null);` resolves, and so do `call`, `__invoke` and the rest.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_first_class_callable_and_a_closure_literal_both_name_closure() {
    check_view(
        r#"
//- /vendor/php/stubs.php
<?php

class Closure
{
    public function bindTo(?object $newThis, object|string|null $newScope = 'static'): ?Closure {}
}

function strlen(string $string): int {}

//- /src/app.php
<?php
namespace App;

class Host
{
    public function fromCallable(): void
    {
        $fn = \strlen(...);
        $f@fcc@n;
    }

    public function fromLiteral(): void
    {
        $fn = function (int $n): int { return $n; };
        $f@literal@n;
    }
}
"#,
        &[
            Query::variable_class("a first-class callable", "fcc", "fn"),
            Query::variable_class("a closure literal", "literal", "fn"),
        ],
        expect![[r#"
            a first-class callable
            - Closure
            a closure literal
            - Closure
        "#]],
    );
}

/// A parameter written as a plain `callable` may be a string or a two-element array, and names no
/// class -- which is the answer it had and keeps.
#[test]
fn a_declared_callable_still_names_no_class() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Host
{
    public function run(callable $fn): void
    {
        $f@declared@n;
    }
}
"#,
        &[Query::variable_class(
            "a declaration, not a value",
            "declared",
            "fn",
        )],
        expect![[r#"
            a declaration, not a value
            - (type not known here)
        "#]],
    );
}
