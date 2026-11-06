//! What a class says it yields through its own methods, having stated nothing in a type argument.
//!
//! PHP's own semantics and not a convention: a `foreach` over an `IteratorAggregate` iterates
//! whatever `getIterator()` gives back, and over an `Iterator` it binds `current()` and `key()`. So
//! a class writing those signatures has said what it yields as surely as an `@implements` tag
//! would, and far more often — `@implements` is written for static analysers, and all three of one
//! real project's own collections write `@return \Traversable<string, X>` on `getIterator()` and
//! none of them writes the tag.
//!
//! Its own file rather than more of [`super::iterated_value`], which is about reading a type
//! argument: nothing here has one to read, and both halves of every answer come from a signature.

use expect_test::expect;

use super::iterated_value::check_iterated;

/// The collection a `getIterator()` return type has to be read through.
///
/// Written as a `\Traversable<K, V>` rather than an `ArrayIterator`, because that is what real code
/// writes and because it is the shape that forced the walk to be recursive: the answer is not on
/// `Bag` and not on `getIterator`, it is on the interface the return type names.
const BAG: &str = r#"
//- /src/Bag.php
<?php
namespace App;

class Row {}

class Bag implements \IteratorAggregate
{
    /** @return \Traversable<string, Row> */
    public function getIterator(): \Traversable
    {
        return new \ArrayIterator([]);
    }
}

class Uses
{
    public function run(Bag $bag): void
    {
        foreach ($bag as $key => $row) {
            $k@key@ey;
            $r@value@ow;
        }
    }
}
"#;

#[test]
fn an_aggregate_yields_what_its_own_get_iterator_returns() {
    check_iterated(
        BAG,
        &["key", "value"],
        expect![[r#"
            key: string
            value: Row
        "#]],
    );
}

#[test]
fn an_iterator_yields_what_its_own_current_and_key_return() {
    // The plainest spelling there is — two native return types, no docblock anywhere — and it
    // answered nothing at all before this walk existed.
    check_iterated(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Row {}

class Rows implements \Iterator
{
    public function current(): Row { return new Row(); }
    public function key(): string { return ''; }
    public function next(): void {}
    public function rewind(): void {}
    public function valid(): bool { return true; }
}

class Uses
{
    public function run(Rows $rows): void
    {
        foreach ($rows as $key => $row) {
            $k@key@ey;
            $r@value@ow;
        }
    }
}
"#,
        &["key", "value"],
        expect![[r#"
            key: string
            value: Row
        "#]],
    );
}

#[test]
fn the_null_an_iterator_returns_past_the_end_is_not_in_the_loop() {
    // `current()` is declared for calling it, and a `foreach` is not that: PHP calls `rewind()`,
    // then `valid()`, and reads `current()` only while it answered true. WordPress writes
    // `@return WP_Block|null` on `WP_Block_List::current()` for exactly the out-of-range case, and
    // reading the null reported 29 `possibly-null-receiver` on loop bodies that cannot see one.
    check_iterated(
        r#"
//- /src/Blocks.php
<?php
namespace App;

class Block {}

class Blocks implements \Iterator
{
    /** @return Block|null */
    public function current(): mixed { return null; }
    /** @return int|null */
    public function key(): mixed { return null; }
    public function next(): void {}
    public function rewind(): void {}
    public function valid(): bool { return true; }
}

class Uses
{
    public function run(Blocks $blocks): void
    {
        foreach ($blocks as $key => $block) {
            $k@key@ey;
            $b@value@lock;
        }
    }
}
"#,
        &["key", "value"],
        expect![[r#"
            key: int
            value: Block
        "#]],
    );
}

#[test]
fn a_type_argument_still_wins_over_the_signature() {
    // The tag is the more specific claim: it says what *this* collection holds where the signature
    // says what the method is declared to return, which for a container written once and reused is
    // the general case.
    check_iterated(
        r#"
//- /src/Special.php
<?php
namespace App;

class Row {}
class Special extends Row {}

/** @implements \IteratorAggregate<int, Special> */
class Bag implements \IteratorAggregate
{
    /** @return \Traversable<string, Row> */
    public function getIterator(): \Traversable
    {
        return new \ArrayIterator([]);
    }
}

class Uses
{
    public function run(Bag $bag): void
    {
        foreach ($bag as $key => $row) {
            $k@key@ey;
            $r@value@ow;
        }
    }
}
"#,
        &["key", "value"],
        expect![[r#"
            key: int
            value: Special
        "#]],
    );
}

#[test]
fn a_class_that_says_nothing_yields_nothing() {
    // The answer that has to survive: a `getIterator()` returning a bare `\Traversable` states no
    // pair, and inventing one is how a stub's own `@template V` came back as a class called `V`.
    check_iterated(
        r#"
//- /src/Silent.php
<?php
namespace App;

class Silent implements \IteratorAggregate
{
    public function getIterator(): \Traversable
    {
        return new \ArrayIterator([]);
    }
}

class Uses
{
    public function run(Silent $silent): void
    {
        foreach ($silent as $key => $row) {
            $k@key@ey;
            $r@value@ow;
        }
    }
}
"#,
        &["key", "value"],
        expect![[r#"
            key: (no type)
            value: (no type)
        "#]],
    );
}
