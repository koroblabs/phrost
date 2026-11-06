//! Completing through a factory whose declared result is a class *with* arguments.
//!
//! The chain is where this has to be asked as well as the path, and the two are genuinely different
//! readers: a statement being typed does not parse, so the chain has only the characters before the
//! cursor. What a call named there is a string, which is exactly the one thing a `class-string<T>`
//! argument needs -- see `IndexView::returned_applied_written`.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_chain_through_a_factory_reaches_what_the_container_was_told_to_hold() {
    check(
        r#"
//- /src/Factory.php
<?php
namespace App;

class Widget
{
    public function doSomething(): void {}
}

/** @template T */
class Bag
{
    /** @return T */
    public function unwrap() {}
}

class Factory
{
    /**
     * @template T
     * @param class-string<T> $class
     * @return Bag<T>
     */
    public function bagOf(string $class) {}

    public function run(): void
    {
        $this->bagOf(Widget::class)->unwrap()->@here@
    }
}
"#,
        &[Query::completion("two links on", "here")],
        expect![[r#"
            two links on
            - doSomething [method]  : void  writes `();`
            - accepting the first leaves `$this->bagOf(Widget::class)->unwrap()->doSomething`
        "#]],
    );
}

#[test]
fn the_container_itself_is_offered_one_link_earlier() {
    check(
        r#"
//- /src/Factory.php
<?php
namespace App;

class Widget {}

/** @template T */
class Bag
{
    /** @return T */
    public function unwrap() {}

    public function count(): int {}
}

class Factory
{
    /**
     * @template T
     * @param class-string<T> $class
     * @return Bag<T>
     */
    public function bagOf(string $class) {}

    public function run(): void
    {
        $this->bagOf(Widget::class)->@here@
    }
}
"#,
        &[Query::completion("the container", "here")],
        // `unwrap` still shows the `T` its declaration wrote: the detail beside a completion row is
        // the member's own type, not what this particular receiver makes of it.
        expect![[r#"
            the container
            - count [method]  : int  writes `();`
            - unwrap [method]  : T  writes `();`
            - accepting the first leaves `$this->bagOf(Widget::class)->count`
        "#]],
    );
}
