//! What one value holds, as against what every value of its class holds.
//!
//! `@param Collection<Item> $items` says what *this* container holds. The declaration's own
//! `@extends`/`@implements` says what every value of the class holds, and a `@template`'s bound says
//! only what every argument it could be given has in common -- so the argument written at the value
//! is the most specific of the three and the only one that answers `$items->get()`.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_declared_argument_answers_the_member_that_returns_the_template() {
    check(
        r#"
//- /src/Declared.php
<?php
namespace App;

class Item
{
    public function label(): string {}
}

/**
 * @template T
 */
interface Collection
{
    /** @return T */
    public function get();
}

class Runner
{
    /** @param Collection<Item> $items */
    public function run(Collection $items): void
    {
        $one = $items->get();
        $one->@member@
    }
}
"#,
        &[Query::completion("what this collection holds", "member")],
        expect![[r#"
            what this collection holds
            - label [method]  : string  writes `();`
            - accepting the first leaves `$one->label`
        "#]],
    );
}

/// `@template T` on the class and `@param T $item` on its constructor: the argument at the `new` is
/// what `T` is for this value and for nothing else of the same class.
#[test]
fn a_constructor_argument_binds_the_classs_template() {
    check(
        r#"
//- /src/Constructed.php
<?php
namespace App;

class Item
{
    public function label(): string {}
}

/**
 * @template T
 */
class Box
{
    /** @param T $item */
    public function __construct($item) {}

    /** @return T */
    public function open() {}
}

class Runner
{
    public function run(Item $item): void
    {
        $box = new Box($item);
        $b@hover@ox;
        $one = $box->open();
        $one->@member@
    }
}
"#,
        &[
            Query::hover("the instance it made", "hover"),
            Query::completion("what it opens to", "member"),
        ],
        expect![[r#"
            the instance it made
            - code: Box<Item> $box
            - note: assigned
            what it opens to
            - label [method]  : string  writes `();`
            - accepting the first leaves `$one->label`
        "#]],
    );
}

/// The same claim through a container parameter: `@param T[] $items` is bound by what the argument
/// holds rather than by the argument.
#[test]
fn a_constructor_container_argument_binds_through_its_element() {
    check(
        r#"
//- /src/Many.php
<?php
namespace App;

class Item
{
    public function label(): string {}
}

/**
 * @template T
 */
class Many
{
    /** @param T[] $items */
    public function __construct(array $items) {}

    /** @return T */
    public function first() {}
}

class Runner
{
    /** @param Item[] $items */
    public function run(array $items): void
    {
        $many = new Many($items);
        $one = $many->first();
        $one->@member@
    }
}
"#,
        &[Query::completion("what the list held", "member")],
        expect![[r#"
            what the list held
            - label [method]  : string  writes `();`
            - accepting the first leaves `$one->label`
        "#]],
    );
}
