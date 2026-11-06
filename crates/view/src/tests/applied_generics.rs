//! A value that is a class *with* arguments, and what comes back out of it.
//!
//! Two ways one is built, and they are the two halves of the same claim. A `@return Bar<T>` beside a
//! `@param class-string<T>` is decided at the call, and a `@return Collection<T>` on a `@template T`
//! class is decided by whatever the receiver was given. Both produce an instance carrying its
//! argument, because that is the only shape a reader can build: an interned `Bar<Widget>` would need
//! the type table open for writing.
//!
//! Owned here rather than only in completion, because the substitution is a projection: the feature
//! above asks what a value holds and this decides. Pinning it here keeps the answer when the way it
//! is produced changes.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_class_string_argument_fills_the_returned_containers_template() {
    check_view(
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
        $bag = $this->bagOf(Widget::class);
        $one = $bag->unwrap();
        $b@bag@ag;
        $o@one@ne;
    }
}
"#,
        &[
            Query::variable_class("the container", "bag", "bag"),
            Query::variable_class("what it holds", "one", "one"),
        ],
        expect![[r#"
            the container
            - App\Bag
            what it holds
            - App\Widget
        "#]],
    );
}

#[test]
fn a_constructors_argument_reaches_a_container_the_method_returns() {
    // The other direction: nothing at the `foobar()` call says anything, and the `@template T` it
    // puts inside `Items` is the one the `new` bound two steps earlier.
    check_view(
        r#"
//- /src/Holder.php
<?php
namespace App;

class Entity
{
    public function doWork(): void {}
}

/** @template T */
interface Items
{
    /** @return T */
    public function get();
}

/** @template T */
class Holder
{
    /** @param T $seed */
    public function __construct($seed) {}

    /** @return Items<T> */
    public function items(): Items {}
}

class Runner
{
    public function run(): void
    {
        $holder = new Holder(new Entity());
        $items = $holder->items();
        $found = $items->get();
        $i@items@tems;
        $f@found@ound;
    }
}
"#,
        &[
            Query::variable_class("the container", "items", "items"),
            Query::variable_class("what it holds", "found", "found"),
        ],
        expect![[r#"
            the container
            - App\Items
            what it holds
            - App\Entity
        "#]],
    );
}

#[test]
fn an_argument_that_settles_nothing_still_names_the_class() {
    // `$name` is a string nobody wrote a class into, so the slot has no answer -- and the contract
    // still knows the result is a `Bag`. Its own case because the alternative used to be nothing at
    // all: the declared return type is refused at lowering for naming a template.
    check_view(
        r#"
//- /src/Loose.php
<?php
namespace App;

/** @template T */
class Bag {}

class Factory
{
    /**
     * @template T
     * @param class-string<T> $class
     * @return Bag<T>
     */
    public function bagOf(string $class) {}

    public function run(string $name): void
    {
        $bag = $this->bagOf($name);
        $b@bag@ag;
    }
}
"#,
        &[Query::variable_class("still a bag", "bag", "bag")],
        expect![[r#"
            still a bag
            - App\Bag
        "#]],
    );
}

/// A class-level `@template` filled from a `class-string<T>` constructor argument.
///
/// The factory shape, and the commonest generic in modern PHP: a container, a repository, a
/// `createMock`. `@param T $item` worked and this did not, and the reason is that the two are
/// different questions about the same parameter -- one asks what the argument *is* and this asks what
/// it *names*. Read as a container it asked for the element of a string and answered nothing, so the
/// template fell back to its bound and every method on the instance returned `object`.
///
/// The class comes from the argument *expression* and not from a literal list, because a `new` carries
/// none: a method step keeps its leading literals and `PathRoot::Instantiated` keeps only the call.
/// That also means `static::class` and an imported alias answer with no case of their own.
///
/// Both spellings, because the second is where it was reported from: the instance held in a variable
/// and the method called a statement later.
#[test]
fn a_class_string_constructor_argument_fills_the_classes_template() {
    check_view(
        r#"
//- /src/classes.php
<?php
namespace App;

class Row
{
    public function label(): string
    {
        return '';
    }
}

/** @template T of object */
class Bag
{
    /** @param class-string<T> $name */
    public function __construct(string $name) {}

    /** @return T */
    public function make(): object {}

    /** @return T */
    public function bare() {}
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): void
    {
        $direct = (new Bag(Row::class))->make();
        $di@direct@rect;
        $held = new Bag(Row::class);
        $one = $held->make();
        $o@one@ne;
        $two = $held->bare();
        $t@two@wo;
        $unfilled = new Bag($runtime);
        $three = $unfilled->make();
        $th@three@ree;
    }
}
"#,
        &[
            Query::variable_type("straight off the new", "direct"),
            Query::variable_type("through a variable", "one"),
            Query::variable_type("with no native return either", "two"),
            Query::variable_type("a name nothing settles", "three"),
        ],
        expect![[r#"
            straight off the new
            - Row
            through a variable
            - Row
            with no native return either
            - Row
            a name nothing settles
            - object
        "#]],
    );
}

/// `@return self<T>` is the same claim as `@return Collection<T>`, and the fluent method of every
/// collection class writes the first.
///
/// Two spellings of one thing, §8.1: the named base was substituted and the relative one resolved
/// to no class at all, so `$c->map(...)` answered `Collection<T>` with the template still in it and
/// the next call in the chain had nothing to go on. `$c->first()` was right the whole time, which is
/// what made it read as a chain problem rather than a substitution one.
#[test]
fn a_self_with_arguments_carries_what_the_receiver_holds() {
    check_view(
        r#"
//- /src/Collection.php
<?php
namespace App;

class Row
{
    public function label(): string {}
}

/** @template T */
class Collection
{
    /**
     * @param callable(T): T $fn
     * @return self<T>
     */
    public function map(callable $fn): self {}

    /** @return T|null */
    public function first() {}
}

class Uses
{
    /** @param Collection<Row> $c */
    public function run(Collection $c): void
    {
        $mapped = $c->map(fn (Row $r) => $r);
        $one = $mapped->first();
        $m@mapped@apped;
        $o@one@ne;
    }
}
"#,
        &[
            Query::variable_class("what map gave back", "mapped", "mapped"),
            Query::variable_class("and what that holds", "one", "one"),
        ],
        expect![[r#"
            what map gave back
            - App\Collection
            and what that holds
            - App\Row
        "#]],
    );
}

#[test]
fn a_list_of_them_is_the_reflection_apis_whole_attribute_idiom() {
    // `getAttributes(Tag::class)[0]->newInstance()`: the stubs declare `@return
    // list<ReflectionAttribute<A>>`, one container out from `Bag<T>`, and the class-string still
    // decides what every element holds.
    check_view(
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
}

class Factory
{
    /**
     * @template T
     * @param class-string<T> $class
     * @return list<Bag<T>>
     */
    public function bagsOf(string $class): array {}

    public function run(): void
    {
        $bags = $this->bagsOf(Widget::class);
        $one = $bags[0]->unwrap();
        $b@bags@ags;
        $o@one@ne;
    }
}
"#,
        &[
            Query::variable_type("the list", "bags"),
            Query::variable_class("what one element holds", "one", "one"),
        ],
        expect![[r#"
            the list
            - Bag<Widget>[]
            what one element holds
            - App\Widget
        "#]],
    );
}
