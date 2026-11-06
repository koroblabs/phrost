//! `iterable` and `callable` are not families of their own, and equality was the wrong test.
//!
//! An **array is iterable**, so `is_iterable($x)` on an `array<int, string>|string` keeps the array
//! and drops the string. Read as `kind == Iterable` it dropped *both* -- nothing survived, so the
//! guard became the whole answer and the array's element type went with it: the branch that plainly
//! knows what it is iterating typed it `iterable`.
//!
//! An **object may be** iterable or callable and may not: which it is depends on a `Traversable` or
//! an `__invoke`, and its class is asked. One that declares `__invoke` is callable and leaves the
//! `!is_callable` branch; one that declares neither may still have a subclass that does, so it
//! survives *either* branch -- which is why the relation has three answers rather than two, and
//! dropping it from the negative side would claim every object callable. A *string* is the same case
//! for `callable`, because `'strlen'` is one, which is why the negative branch keeps it.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /stubs/standard.php  stub
<?php
function is_iterable(mixed $value): bool {}
function is_callable(mixed $value): bool {}

//- /src/Kinds.php
<?php
namespace App;

class Handler
{
    public function __invoke(): void {}
}

class Kinds
{
    /** @param array<int, string>|string $command */
    public function iterating($command): void
    {
        if (is_iterable($command)) {
            $com@iterable@mand;
        }
    }

    /** @param array<int, string>|string $command */
    public function notIterating($command): void
    {
        if (!is_iterable($command)) {
            $com@rest@mand;
        }
    }

    /** @param Handler|string $handler */
    public function invoking($handler): void
    {
        if (is_callable($handler)) {
            $han@callable@dler;
        }
    }

    /** @param Handler|string $handler */
    public function notInvoking($handler): void
    {
        if (!is_callable($handler)) {
            $han@neither@dler;
        }
    }
}
"#;

/// Both branches of both tests, because the three-answer relation is only right if a `Maybe`
/// survives each of them.
#[test]
fn an_array_answers_a_test_for_iterable_and_an_object_answers_neither_way() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("an array is iterable, and keeps what it holds", "iterable"),
            Query::variable_type("and the string is what is left over", "rest"),
            Query::variable_type("an object may be callable, so it stays", "callable"),
            Query::variable_type(
                "and a string, which may name a function -- the invokable class does not",
                "neither",
            ),
        ],
        expect![[r#"
            an array is iterable, and keeps what it holds
            - array<int, string>
            and the string is what is left over
            - string
            an object may be callable, so it stays
            - Handler|string
            and a string, which may name a function -- the invokable class does not
            - string
        "#]],
    );
}
