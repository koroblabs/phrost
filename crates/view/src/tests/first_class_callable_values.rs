//! What a first-class callable holds, and what survives being called.
//!
//! The layer that owns the answer, the same way [`super::closure_invocation`] owns it for a closure
//! literal: the body says the value is a callable wrapped around the call that was never made, and
//! this is where the callee's return type is looked up -- which needs a catalog, because the callee
//! is usually declared in another file.
//!
//! The chain is what is actually being asserted. `$fn = $this->probe(...)` answering is worth
//! little on its own; `$fn(1)->title()` reaching `string` is the whole point, because before this
//! the shape produced no value at all and **everything downstream of it was dead**. So every test
//! here asks about a step past the call rather than only about the callable.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_method_callable_hands_back_what_the_method_returns() {
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function title(): string {}
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function probe(int $n): Row {}

    public function go(): void
    {
        $fn = $this->probe(...);
        $row = $fn(1);
        $ti@titled@tle = $row->title();
        $r@row@ow;
    }
}
"#,
        &[
            Query::variable_class("what calling it gives", "row", "row"),
            Query::variable_type("the step past the call", "titled"),
        ],
        expect![[r#"
            what calling it gives
            - App\Row
            the step past the call
            - string
        "#]],
    );
}

#[test]
fn a_static_callable_is_read_through_the_class_it_names() {
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function title(): string {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public static function find(int $id): Row {}
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(): void
    {
        $fn = Repo::find(...);
        $row = $fn(1);
        $r@row@ow;
    }
}
"#,
        &[Query::variable_class("through the class", "row", "row")],
        expect![[r#"
            through the class
            - App\Row
        "#]],
    );
}

#[test]
fn a_function_callable_hands_back_what_the_function_returns() {
    // No receiver and so no member site -- the name is a reference in the function table, and the
    // value has to come from the same place a plain call's return does.
    check_view(
        r#"
//- /src/Runner.php
<?php
namespace App;

function measure(string $text): int {}

class Runner
{
    public function go(): void
    {
        $fn = measure(...);
        $n@count@ = $fn('a');
    }
}
"#,
        &[Query::variable_type("the function's return", "count")],
        expect![[r#"
            the function's return
            - int
        "#]],
    );
}

#[test]
fn the_callable_itself_is_a_callable_and_not_the_return_type() {
    // The assertion the wrapper exists for. `$fn` is a `Closure` that *produces* a `Row`; reading
    // it as a `Row` would make `$fn->title()` offer a method the value does not have, and would
    // make `$fn(1)` a call on an object with no `__invoke`.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function title(): string {}
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function probe(int $n): Row {}

    public function go(): void
    {
        $fn = $this->probe(...);
        $f@held@n;
    }
}
"#,
        &[
            Query::variable_type("rendered", "held"),
            Query::variable_class("names no class", "held", "fn"),
        ],
        expect![[r#"
            rendered
            - Closure(...): Row
            names no class
            - (type not known here)
        "#]],
    );
}

#[test]
fn a_callee_that_declares_no_return_leaves_a_callable_with_no_result() {
    // The honest miss, and the same shape the closure arm has: still a callable, because that is
    // true and is a smaller claim than the nothing this used to give.
    check_view(
        r#"
//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function probe($n) {}

    public function go(): void
    {
        $fn = $this->probe(...);
        $q = $fn(1);
        $@called@q;
    }
}
"#,
        &[Query::variable_class("unknown result", "called", "q")],
        expect![[r#"
            unknown result
            - (type not known here)
        "#]],
    );
}
