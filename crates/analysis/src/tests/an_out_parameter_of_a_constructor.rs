//! A constructor's by-reference parameter binds the variable exactly as a function's does.
//!
//! `new Foo($bar)` where `__construct(Baz &$bar)` is how a builder hands something back through an
//! argument, and it answered nothing while `fill($bar)` beside it answered `Baz`. The body already
//! bound the name off the instantiation -- `passed_by_reference` reads `new` -- but the reader then
//! walks every call that passed the value to find the one whose parameter is really by-reference,
//! and that walk knew a method, a static call and a function and not a `new`. One half of a rule in
//! two places, the shape §8 records for `local_binding` against `bound_names`.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_variable_passed_to_a_constructor_by_reference_takes_the_parameters_type() {
    check(
        r#"
//- /src/app.php
<?php

class Baz
{
    public function doWork(): void {}
}

class Foo
{
    public function __construct(Baz &$bar)
    {
        $bar = new Baz();
    }
}

function fill(Baz &$out): void
{
    $out = new Baz();
}

function run(): void
{
    new Foo($built);
    $bu@constructed@ilt->doWork();
    fill($filled);
    $fi@called@lled->doWork();
}
"#,
        &[
            Query::hover("after the constructor", "constructed"),
            Query::hover("after the function, the control", "called"),
        ],
        expect![[r#"
            after the constructor
            - code: Baz $built
            - note: assigned
            after the function, the control
            - code: Baz $filled
            - note: assigned
        "#]],
    );
}
