//! A constructor parameter declared as a class template is judged against the template's bound.
//!
//! `new` binds the class's templates and nothing has supplied them yet, so `T` is as much as the
//! bound promises -- `new Collection(1)` under `@template T of array` is outside it. Without the
//! class's templates a constructor's `T` named nothing and every argument for it was silence, which
//! a method on the same class never was.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_argument_outside_the_bound_is_reported_and_one_inside_is_not() {
    check(
        r#"
//- /src/Collection.php
<?php
namespace App;

/** @template T of array */
final class Collection
{
    /** @param T $items */
    public function __construct(public $items) {}
}

/** @template T */
final class Anything
{
    /** @param T $value */
    public function __construct(public $value) {}
}

function go(): void
{
    new Collection([1, 2]);
    new Collection(1);
    new Anything(1);
}
"#,
        expect![[r#"
            21:20-21:21 error [incompatible-argument] $items takes array, and this is int
        "#]],
    );
}
