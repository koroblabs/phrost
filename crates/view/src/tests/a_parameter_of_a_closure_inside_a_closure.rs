//! A parameter declared by a closure that is itself inside a closure.
//!
//! Laravel's `bootstrap/app.php` is this shape: a closure handed to a builder, and an arrow
//! function handed to something inside it. Every use of the inner parameter answered, and its own
//! name did not -- the search for parameters listed the closures of each declaration's body and not
//! the closures of a closure's.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_inner_parameter_answers_at_its_own_name() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Request {}

$configure = function (array $rules): void {
    $rules[] = fn (Request $req@declared@uest) => $req@used@uest;
};
"#,
        &[
            Query::variable_type("at the declaration", "declared"),
            Query::variable_type("at a use", "used"),
        ],
        expect![[r#"
            at the declaration
            - Request
            at a use
            - Request
        "#]],
    );
}
