//! What a loop variable holds.
//!
//! The most common way a local gets a type in real code, and the one that needed type arguments:
//! `foreach` binds a name to the *element* of what it iterates, and an element only exists if the
//! type says what it contains.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_foreach_over_a_documented_list_types_its_variable() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    /** @var list<Repo> */
    private $all;

    public function run(): void
    {
        foreach ($this->all as $one) {
            $one->@here@
        }
    }
}
"#,
        &[Query::completion("over a list", "here")],
        expect![[r#"
            over a list
            - find [method]  : void  writes `();`
            - accepting the first leaves `$one->find`
        "#]],
    );
}

#[test]
fn every_way_of_writing_an_array_of_something_works() {
    // `Repo[]`, `array<int, Repo>`, `iterable<Repo>` and `list<Repo>` are the same claim written
    // four ways, and the element is the last argument in each.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    /**
     * @param Repo[] $slice
     * @param array<int, Repo> $keyed
     * @param iterable<Repo> $iterable
     */
    public function run($slice, $keyed, $iterable): void
    {
        foreach ($slice as $a) {
            $a->@slice@
        }
        foreach ($keyed as $b) {
            $b->@keyed@
        }
        foreach ($iterable as $c) {
            $c->@iterable@
        }
    }
}
"#,
        &[
            Query::completion("Repo[]", "slice"),
            Query::completion("array<int, Repo>", "keyed"),
            Query::completion("iterable<Repo>", "iterable"),
        ],
        expect![[r#"
            Repo[]
            - find [method]  : void  writes `();`
            - accepting the first leaves `$a->find`
            array<int, Repo>
            - find [method]  : void  writes `();`
            - accepting the first leaves `$b->find`
            iterable<Repo>
            - find [method]  : void  writes `();`
            - accepting the first leaves `$c->find`
        "#]],
    );
}

#[test]
fn a_foreach_over_a_method_result_reaches_through_the_chain() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    /** @return list<Repo> */
    public function all()
    {
    }

    public function run(): void
    {
        foreach ($this->all() as $one) {
            $one->@here@
        }
    }
}
"#,
        &[Query::completion("through a chain", "here")],
        expect![[r#"
            through a chain
            - find [method]  : void  writes `();`
            - accepting the first leaves `$one->find`
        "#]],
    );
}

#[test]
fn a_foreach_over_an_array_that_says_nothing_offers_nothing() {
    // A bare `array` does not say what is in it. Typing the loop variable anyway would be inventing
    // a class from the shape of the code.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}

    public function run(array $items): void
    {
        foreach ($items as $one) {
            $one->@here@
        }
    }
}
"#,
        &[Query::completion("bare array", "here")],
        expect![[r#"
            bare array
            - (nothing offered)
        "#]],
    );
}

#[test]
fn the_key_of_a_foreach_is_bound_without_being_typed() {
    // `array<K, V>` says what the key is too, but the element step answers about the value. Naming
    // the key's type needs the other half, and claiming the value's for it would be wrong.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    /** @param array<string, Repo> $items */
    public function theKey($items): void
    {
        foreach ($items as $key => $one) {
            $key->@key@
        }
    }

    /** @param array<string, Repo> $items */
    public function theValue($items): void
    {
        foreach ($items as $key => $one) {
            $one->@value@
        }
    }
}
"#,
        &[
            Query::completion("the key", "key"),
            Query::completion("the value", "value"),
        ],
        expect![[r#"
            the key
            - (nothing offered)
            the value
            - find [method]  : void  writes `();`
            - accepting the first leaves `$one->find`
        "#]],
    );
}

#[test]
fn a_generic_class_keeps_its_own_members_as_well_as_its_element() {
    // `Collection<Repo>` is both: a class you can call methods on, and something you can iterate --
    // which it can only be by being `Traversable`, since any other object hands a loop its
    // properties.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

/** @template T */
class Collection implements \IteratorAggregate
{
    public function count(): int {}
}

class Service
{
    /** @var Collection<Repo> */
    private $items;

    public function theCollection(): void
    {
        $this->items->@collection@
    }

    public function anElement(): void
    {
        foreach ($this->items as $one) {
            $one->@element@
        }
    }
}
"#,
        &[
            Query::completion("the collection", "collection"),
            Query::completion("an element", "element"),
        ],
        expect![[r#"
            the collection
            - count [method]  : int  writes `();`
            - accepting the first leaves `$this->items->count`
            an element
            - find [method]  : void  writes `();`
            - accepting the first leaves `$one->find`
        "#]],
    );
}

#[test]
fn a_class_says_what_it_holds_through_a_template_bound() {
    // The only way a PHP container can say it: the language has no generics, so `@template` in a
    // docblock is the whole statement. Drupal's field lists are written exactly like this, and
    // iterating one used to give the loop variable no type at all.
    check(
        r#"
//- /src/Item.php
<?php
namespace App;

interface Item
{
    public function value(): string;
}

//- /src/Items.php
<?php
namespace App;

/**
 * A list of items.
 *
 * @template K of int|string
 * @template V of Item
 */
interface Items extends \IteratorAggregate {}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function run(Items $items): void
    {
        foreach ($items as $one) {
            $one->val@here@ue();
        }
    }
}
"#,
        &[Query::goto("through the bound", "here")],
        expect![[r#"
            through the bound
            - method App\Item::value @ src/Item.php 6:21-6:26 (full 6:5-6:37)
        "#]],
    );
}

#[test]
fn a_written_type_argument_beats_the_bound() {
    // The order the two sources are asked in. An argument says what *this* value holds; a bound
    // says what any value of the type could hold, so the argument has to win wherever it exists --
    // and here it names a subclass the bound does not.
    check(
        r#"
//- /src/Item.php
<?php
namespace App;

interface Item {}

class Special implements Item
{
    public function only(): void {}
}

//- /src/Items.php
<?php
namespace App;

/**
 * @template V of Item
 */
interface Items extends \IteratorAggregate {}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    /** @param Items<Special> $items */
    public function run(Items $items): void
    {
        foreach ($items as $one) {
            $one->on@here@ly();
        }
    }
}
"#,
        &[Query::goto("the argument wins", "here")],
        expect![[r#"
            the argument wins
            - method App\Special::only @ src/Item.php 8:21-8:25 (full 8:5-8:36)
        "#]],
    );
}

#[test]
fn a_class_with_no_template_still_yields_nothing() {
    check(
        r#"
//- /src/Bag.php
<?php
namespace App;

class Bag implements \IteratorAggregate {}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function run(Bag $bag): void
    {
        foreach ($bag as $one) {
            $one->any@here@thing();
        }
    }
}
"#,
        &[Query::goto("nothing said", "here")],
        expect![[r#"
            nothing said
            - (nothing to go to)
        "#]],
    );
}

#[test]
fn the_key_of_a_foreach_is_typed_by_how_the_container_was_written() {
    // PHP's rules, and they are about the container rather than what is in it. `array<K, V>` writes
    // its key half down; anything with one argument is PHP's implicit numbering; a shape names its
    // keys individually.
    check(
        r#"
//- /src/Keys.php
<?php
namespace App;

class Keys
{
    /**
     * @param array<string, int> $map
     * @param list<int> $xs
     * @param array{name: string, city: string} $row
     * @param array $bare
     */
    public function run(array $map, array $xs, array $row, array $bare): void
    {
        foreach ($map as $mapKey => $one) {
            $map@one@Key;
        }
        foreach ($xs as $index => $two) {
            $ind@two@ex;
        }
        foreach ($row as $field => $three) {
            $fie@three@ld;
        }
        foreach ($bare as $unknown => $four) {
            $unkn@four@own;
        }
    }
}
"#,
        &[
            Query::hover("a map's key", "one"),
            Query::hover("a list's index", "two"),
            Query::hover("a shape's field name", "three"),
            Query::hover("a bare array's key", "four"),
        ],
        expect![[r#"
            a map's key
            - code: string $mapKey
            - note: loop variable
            a list's index
            - code: int $index
            - note: loop variable
            a shape's field name
            - code: non-falsy-string $field
            - note: loop variable
            a bare array's key
            - code: $unknown
            - note: loop variable
            - note: no declared type
        "#]],
    );
}

/// `@implements IteratorAggregate<Command>` gives one argument to a two-parameter interface, and it
/// means the value. `<Key, Value>` is the only pair anybody writes, so one argument is the second
/// half of it.
#[test]
fn one_argument_to_a_key_value_container_is_the_value() {
    check(
        r#"
//- /src/stubs.php
<?php

/**
 * @template-covariant K
 * @template-covariant V
 */
interface Traversable {}

/**
 * @template-covariant K
 * @template-covariant V
 *
 * @extends Traversable<K, V>
 */
interface IteratorAggregate extends Traversable
{
    /** @return Traversable<K, V> */
    public function getIterator(): Traversable;
}

//- /src/Queue.php
<?php

class Command
{
    public function execute(): void {}
}

/**
 * @implements IteratorAggregate<Command>
 */
class CommandQueue implements IteratorAggregate {}

class Runner
{
    public function run(CommandQueue $queue): void
    {
        foreach ($queue as $one) {
            $one->@member@
        }
    }
}
"#,
        &[Query::completion("what the queue yields", "member")],
        expect![[r#"
            what the queue yields
            - execute [method]  : void  writes `();`
            - accepting the first leaves `$one->execute`
        "#]],
    );
}
