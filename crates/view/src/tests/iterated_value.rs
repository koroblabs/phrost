//! Which type argument a `foreach` binds, when the container names one of PHP's own contracts.
//!
//! Its own file because every question here is about a value that names *no class* — a `string`, an
//! array shape — which is the whole reason the answer had to stop being a class name. The shared
//! `Query::variable_class` cannot ask it: it reduces to a class and would report the same nothing
//! for a right answer and a missing one.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;

use crate::IndexView;

/// PHP's iteration interfaces as the shipped prelude declares them.
///
/// Written into each fixture rather than assumed, because the value slot is read off *these*
/// template lists: `Iterator<TKey, TValue>` is two parameters with the value second, and a test that
/// did not say so would be pinning a rule against nothing.
pub(super) const CONTRACTS: &str = r#"
//- /stubs/core.php
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
interface Iterator extends Traversable
{
    public function current(): mixed;
    public function key(): mixed;
    public function next(): void;
    public function valid(): bool;
    public function rewind(): void;
}

/**
 * @template-covariant K
 * @template-covariant V
 *
 * @extends Traversable<K, V>
 */
interface IteratorAggregate extends Traversable
{
    public function getIterator(): Traversable;
}
"#;

/// The rendered type of the variable each marker stands on.
///
/// Through the open-versus-closed dimension, the same as every other view fixture: a cross-file
/// answer that started depending on which buffers are open fails here.
pub(super) fn check_iterated(fixture: &str, markers: &[&str], expect: Expect) {
    let spec = FixtureSpec::parse(&format!("{CONTRACTS}{fixture}"));
    let world = World::build(&spec);
    let mut out = String::new();

    for marker in markers {
        let position = spec.markers().position(marker);
        let file = world.file_id(&position.path);
        let answer = with_open_scopes(&world, Some(file), |open| {
            let view = IndexView::in_file(world.skeleton(), open, file);
            match view
                .variable_at(position.offset)
                .and_then(|variable| variable.ty)
            {
                Some(ty) => ty,
                None => "(no type)".to_owned(),
            }
        });
        let _ = writeln!(out, "{marker}: {answer}");
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

#[test]
fn the_value_of_an_iterator_is_its_second_argument_and_not_its_last_template() {
    // The shape that makes a positional guess wrong. `Test1` declares one template and hands it to
    // `Iterator` as the *key*, so "the last template of the nearest declaration that has one" finds
    // `int` -- the key `Test2` supplied -- while the value is written two levels up as a literal
    // `string` nothing substitutes.
    //
    // The key is the same fact read the other way round, and it used to answer nothing at all: a
    // class stating its pair had one half of it read. `int` here is what `@extends Rows<int>`
    // supplied through `@implements \Iterator<TKey, string>`, which is as declared as the value is.
    check_iterated(
        r#"
//- /src/Chain.php
<?php
namespace App;

/**
 * @template TKey
 * @implements \Iterator<TKey, string>
 */
abstract class Rows implements \Iterator {}

/**
 * @extends Rows<int>
 */
class IntRows extends Rows {}

class Uses
{
    public function run(IntRows $rows): void
    {
        foreach ($rows as $key => $value) {
            $k@key@ey;
            $va@value@lue;
        }
    }
}
"#,
        &["key", "value"],
        expect![[r#"
            key: int
            value: string
        "#]],
    );
}

#[test]
fn a_single_argument_to_a_two_parameter_contract_is_the_value() {
    // `@implements IteratorAggregate<X>` is how almost every real container is written, and the one
    // argument means the value rather than the key. It is also a shape no class name can express:
    // the element here is an array, so an answer of "the class it is" is no answer at all.
    //
    // The key is the half that has to stay silent, and reading it positionally does not: the one
    // argument lands in the interface's first template and travels down its own
    // `@extends Traversable<K, V>`, so slot zero answers with the value under another name.
    check_iterated(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Foo {}
class Bar {}

/**
 * @implements \IteratorAggregate<array<string, Foo|Bar>>
 */
class Rows implements \IteratorAggregate {}

class Uses
{
    public function run(Rows $rows): void
    {
        foreach ($rows as $key => $row) {
            $k@key@ey;
            $r@row@ow;
        }
    }
}
"#,
        &["key", "row"],
        // The parentheses are the type table's rendering of a union nested inside a composite, not
        // anything this resolved: `(A&B)|C` and `A&(B|C)` would otherwise read the same. They are
        // redundant between `<` and `>`, which is a renderer's question and not this one's.
        expect![[r#"
            key: (no type)
            row: array<string, Foo|Bar>
        "#]],
    );
}

#[test]
fn a_contract_argument_that_names_only_a_template_is_not_an_answer() {
    // The declaration passes its own parameter through and nobody ever fills it. `TValue` is a name
    // resolving to no class, and handing it back would report every member of the loop variable
    // against a class called `TValue`.
    check_iterated(
        r#"
//- /src/Rows.php
<?php
namespace App;

/**
 * @template TValue
 * @implements \IteratorAggregate<int, TValue>
 */
class Rows implements \IteratorAggregate {}

class Uses
{
    public function run(Rows $rows): void
    {
        foreach ($rows as $row) {
            $r@row@ow;
        }
    }
}
"#,
        &["row"],
        expect![[r#"
            row: (no type)
        "#]],
    );
}

#[test]
fn a_template_bound_still_answers_for_a_container_php_does_not_declare() {
    // The Drupal shape, and the one the walk exists for: nothing supplies an argument anywhere, so
    // the bound is what every value of the interface has in common. Guarded here because the
    // contract rule runs first and must not shadow it -- a bound on a user interface is nearer than
    // an empty `IteratorAggregate` above it.
    check_iterated(
        r#"
//- /src/Items.php
<?php
namespace App;

interface Item {}

/**
 * @template K of int|string
 * @template V of Item
 */
interface Items extends \IteratorAggregate {}

class Uses
{
    public function run(Items $items): void
    {
        foreach ($items as $item) {
            $it@item@em;
        }
    }
}
"#,
        &["item"],
        expect![[r#"
            item: Item
        "#]],
    );
}

#[test]
fn an_object_php_does_not_traverse_yields_nothing_a_template_says() {
    // A `foreach` over an object that is not `Traversable` binds its visible properties, so no
    // template of anything it uses or extends says what a pass holds. The convention reached the
    // trait an Eloquent model uses -- `HasFactory<TFactory of Factory>` -- and iterated every model
    // as a factory. Last, the control: a container that *is* traversable keeps the convention.
    check_iterated(
        r#"
//- /src/Models.php
<?php
namespace App;

class Item {}

class Factory {}

/** @template TFactory of Factory */
trait HasFactory {}

class Model
{
    use HasFactory;
}

/** @template TValue */
class Box {}

/** @template TValue */
class Items implements \IteratorAggregate
{
    public function getIterator(): \Iterator {}
}

class Uses
{
    /**
     * @param Box<Item> $box
     * @param Items<Item> $items
     */
    public function run(Model $model, Box $box, Items $items): void
    {
        foreach ($model as $field) {
            $fi@field@eld;
        }
        foreach ($box as $boxed) {
            $bo@boxed@xed;
        }
        foreach ($items as $item) {
            $it@item@em;
        }
    }
}
"#,
        &["field", "boxed", "item"],
        expect![[r#"
            field: (no type)
            boxed: (no type)
            item: Item
        "#]],
    );
}

#[test]
fn an_argument_the_property_supplies_beats_the_bound_the_class_declares() {
    // The other arm of the test above, and the rule the vendored DOM stub is written against. A
    // bound is what answers when nothing supplies an argument, so a property that says nothing is
    // not untyped -- it answers the union, which is a plausible type and the wrong one:
    // `DOMNode::$childNodes` was a bare `DOMNodeList` and every `foreach` over it bound
    // `DOMNode|DOMNameSpaceNode`, reported as an argument mismatch at the `saveHTML($child)`
    // beside it. A namespace node cannot be in a tree at all -- only `DOMXPath::query()` makes one
    // -- so the tag is the honest answer and the bound is the fallback. Both arms in one fixture
    // because they are two constructors of one answer and only one of them was reached.
    check_iterated(
        r#"
//- /src/Nodes.php
<?php
namespace App;

class Node {}

class NamespaceNode {}

/**
 * @template-covariant T of Node|NamespaceNode
 *
 * @template-implements \IteratorAggregate<int, T>
 */
class NodeList implements \IteratorAggregate
{
    /** @return \Iterator<int, T> */
    public function getIterator(): \Iterator {}
}

class Tree
{
    /** @var NodeList<Node> */
    public NodeList $children;

    public NodeList $matched;

    public function run(): void
    {
        foreach ($this->children as $child) {
            $ch@child@ild;
        }
        foreach ($this->matched as $found) {
            $fo@found@und;
        }
    }
}
"#,
        &["child", "found"],
        expect![[r#"
            child: Node
            found: Node|NamespaceNode
        "#]],
    );
}

#[test]
fn a_generator_written_in_full_binds_its_second_argument_and_not_its_last() {
    // The whole reason the contract table names its slots, applied where a type argument is read
    // rather than where a class declares one. `Generator<TKey, TValue, TSend, TReturn>` puts what
    // `getReturn()` gives at the end, so taking the last argument answers the one type a `foreach`
    // never binds — and two readers used to disagree about that, the table knowing and
    // `TypeTable::element_of` taking the last. The same pair iterated as `Row` written on a class
    // and as `void` written on a return.
    check_iterated(
        r#"
//- /src/Stream.php
<?php
namespace App;

class Row {}

class Stream
{
    /** @return \Generator<int, Row, mixed, void> */
    public function rows(): \Generator
    {
        yield new Row();
    }

    public function run(): void
    {
        foreach ($this->rows() as $key => $row) {
            $k@key@ey;
            $r@row@ow;
        }
    }
}
"#,
        &["key", "row"],
        expect![[r#"
            key: int
            row: Row
        "#]],
    );
}
