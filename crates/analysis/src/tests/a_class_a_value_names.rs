//! `new $class()`, where what is known of the class is a bound rather than an identity.
//!
//! `@param class-string<Product> $class` says "`Product`, or any subclass of it". That is how a
//! factory over an abstract base is written, and resolving the bound to `Product` and reporting it
//! turned a correct annotation into `Cannot instantiate abstract class` -- a fatal-looking error on
//! code that runs. PHPStan reads the same tag the same way and says nothing.
//!
//! The line the check now draws is the one `ClassTarget` already drew: a class **named outright** is
//! an identity and a class that arrives as a **value** is not. `new Product()` still reports, and so
//! does `new self()` inside an abstract class.
//!
//! What this used to give up is `$c = Product::class; new $c();`, which is a real error, on the
//! ground that a `::class` literal and a docblock's `class-string<X>` are the same type by the time
//! this can ask. They are -- and the *path* is not: a value whose bindings end at `Product::class`,
//! or at the string `'Product'` PHP wrote before `::class` existed, names that class exactly, and
//! one that ends at a declaration names a bound. So the line is drawn there, and every arm whose
//! answer is a bound -- an instance's class, a `class-string<T>`, a table read by key -- still says
//! nothing.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_class_string_bound_is_not_the_class_it_bounds() {
    check(
        r#"
//- /app.php
<?php
abstract class Product
{
    abstract public function rate(): float;
}

class Factory
{
    /** @param class-string<Product> $class */
    public function make(string $class): Product
    {
        return new $class();
    }

    /**
     * @template T of Product
     * @param class-string<T> $class
     * @return T
     */
    public function makeTemplated(string $class): Product
    {
        return new $class();
    }

    /** The bound travelling through a local changes nothing about what it is. */
    public function throughALocal(string $class): Product
    {
        $held = $class;
        return new $held();
    }

    /** Naming it outright is still an error, and is the whole of what is left. */
    public function named(): Product
    {
        return new Product();
    }
}
"#,
        expect![[r#"
            35:16-35:29 error [uninstantiable-class] Cannot instantiate abstract class Product
        "#]],
    );
}

#[test]
fn a_class_written_into_the_value_is_the_class_it_names() {
    check(
        r#"
//- /app.php
<?php
abstract class Product {}

interface Rated {}

class Concrete extends Product {}

function make(Product $product): void
{
    $abstract = Product::class;
    new $abstract();
    $interface = '\\Rated';
    new $interface();
    $fine = 'Concrete';
    new $fine();
    $instance = $product::class;
    new $instance();
}
"#,
        expect![[r#"
            11:5-11:20 error [uninstantiable-class] Cannot instantiate abstract class Product
            13:5-13:21 error [uninstantiable-class] Cannot instantiate interface Rated
        "#]],
    );
}
