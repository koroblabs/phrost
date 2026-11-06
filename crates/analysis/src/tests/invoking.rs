//! Calling a value rather than a name.
//!
//! PHP's rule is that calling an object calls its `__invoke`, which is how every handler, middleware
//! and single-method service in a modern project is written. The step names no method where it is
//! read, because neither reader has an interner -- the layer that resolves it is the one that knows.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn invoking_a_local_reaches_its_invoke() {
    check(
        r#"
//- /src/Handler.php
<?php
namespace App;

class Formatter
{
    public function format(): string {}
}

class Handler
{
    public function __invoke(): Formatter {}
}

class Runner
{
    public function run(Handler $handler): void
    {
        $handler()->@chained@
    }

    public function bound(Handler $handler): void
    {
        $result = $handler();
        $result->@bound@
    }
}
"#,
        &[
            Query::completion("invoked under the cursor", "chained"),
            Query::completion("invoked and assigned", "bound"),
        ],
        expect![[r#"
            invoked under the cursor
            - format [method]  : string  writes `();`
            - accepting the first leaves `$handler()->format`
            invoked and assigned
            - format [method]  : string  writes `();`
            - accepting the first leaves `$result->format`
        "#]],
    );
}

#[test]
fn invoking_a_property_reaches_its_invoke() {
    check(
        r#"
//- /src/Property.php
<?php
namespace App;

class Formatter
{
    public function format(): string {}
}

class Handler
{
    public function __invoke(): Formatter {}
}

class Runner
{
    private Handler $handler;

    public function run(): void
    {
        $result = ($this->handler)();
        $result->@member@
    }
}
"#,
        &[Query::completion("a property invoked", "member")],
        expect![[r#"
            a property invoked
            - format [method]  : string  writes `();`
            - accepting the first leaves `$result->format`
        "#]],
    );
}

/// A bare `helper()->` is a function's **result**, not a value being invoked: reading it the second
/// way would answer with `__invoke` on whatever the name happened to be.
///
/// It is its own chain root, looked up in the function table rather than resolved to a class the
/// characters name -- which is why this used to offer nothing at all, and why the thing it offers
/// now is what `helper()` was declared to return.
#[test]
fn a_plain_function_call_is_not_an_invoked_value() {
    check(
        r#"
//- /src/Function.php
<?php
namespace App;

class Formatter
{
    public function format(): string {}
}

function helper(): Formatter {}

class Runner
{
    public function run(): void
    {
        helper()->@member@
    }
}
"#,
        &[Query::completion("a function call", "member")],
        expect![[r#"
            a function call
            - format [method]  : string  writes `();`
            - accepting the first leaves `helper()->format`
        "#]],
    );
}
