//! A constructor parameter whose type is a union that names a class template.
//!
//! `ReflectionClass::__construct` is `@param class-string<T>|T $objectOrClass` beside a native
//! `object|string`, and Laravel's collection is `Arrayable<TKey, TValue>|iterable<TKey, TValue>|null`.
//! Only a bare template, a `class-string<T>` or a container of one was read, and only off the
//! effective type -- which is the hint wherever the docblock names something the hint cannot be
//! related to, and a template is exactly that.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_operand_the_argument_was_is_the_one_that_binds() {
    check_view(
        r#"
//- /src/a.php
<?php
namespace App;

class Store {}

/** @template T of object */
class Refl
{
    /** @param class-string<T>|T $objectOrClass */
    public function __construct(object|string $objectOrClass) {}

    /** @return T */
    public function make(): object {}
}

/** @template V */
class Box
{
    /** @param V $value */
    public function __construct($value) {}

    /** @return V */
    public function get() {}
}

function run(Store $s, string $name): void
{
    $n = new Refl($name);
    $@n@n;
    $r = new Refl(Store::class);
    $@r@r;
    $o = new Refl($s);
    $@o@o;
    $b = new Box($s);
    $@b@b;
}
"#,
        &[
            Query::variable_type("a class named by ::class", "r"),
            Query::variable_type("an instance of it", "o"),
            Query::variable_type("a plain template, for comparison", "b"),
            Query::variable_type("a string nothing can read is no instance", "n"),
        ],
        expect![[r#"
            a class named by ::class
            - Refl<Store>
            an instance of it
            - Refl<Store>
            a plain template, for comparison
            - Box<Store>
            a string nothing can read is no instance
            - Refl
        "#]],
    );
}
