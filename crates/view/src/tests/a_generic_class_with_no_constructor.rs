//! `new` of a generic class that declares no constructor is that class.
//!
//! Binding a template reads the constructor's parameters, and a class with no constructor anywhere
//! above it answered *nothing* -- not the plain class, not a guess -- so `new \SplObjectStorage()`,
//! `new \SplQueue()` and every hand-written collection without a constructor left the variable
//! untyped, and everything called on it with no completion and no goto. With no arguments there is
//! nothing to bind, which is exactly the plain class.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_generic_class_with_no_constructor_is_the_class() {
    check_view(
        r#"
//- /src/Bag.php
<?php
namespace App;

/** @template T */
class Bag
{
    /** @param T $item */
    public function add(mixed $item): void {}
}

/** @template T */
class Box
{
    /** @param T $item */
    public function __construct(mixed $item) {}
}

class Runner
{
    public function go(): void
    {
        $b@bag@ag = new Bag();
        $x@box@ox = new Box(1);
    }
}
"#,
        &[
            Query::variable_type("no constructor anywhere", "bag"),
            Query::variable_type("a constructor that binds it", "box"),
        ],
        expect![[r#"
            no constructor anywhere
            - Bag
            a constructor that binds it
            - Box<int>
        "#]],
    );
}
