//! `@phpstan-self-out` / `@phpstan-this-out` / `@psalm-this-out`: after `$x->m(...)` the variable
//! holds what the method says it left there.
//!
//! The receiver's twin of a `@param-out`, read through the same seam -- the calls a value was handed
//! to before a position -- with the receiver as the value handed over. `self` and `static` in the
//! out type are the declaring class and the class the call was made on; a slot the receiver
//! supplies comes from what it was written with, and a method's own template from the call's
//! arguments, exactly as a return's would.

use expect_test::expect;

use super::{Query, check_view};

/// The shape of the third-party conformance test: a box of `int` given a `string`.
const BOX: &str = r#"
//- /src/Box.php
<?php
namespace App;

/** @template T */
final class Box
{
    /** @param T $value */
    public function __construct(public mixed $value) {}

    /**
     * @template U
     * @param U $value
     * @psalm-this-out self<U>
     * @phpstan-self-out self<U>
     */
    public function replace(mixed $value): void {}
}
"#;

/// Laravel's `Collection::transform()`, reduced: the keys are the receiver's own and the values are
/// whatever the callback returns.
const ITEMS: &str = r#"
//- /src/Items.php
<?php
namespace App;

class User { public function name(): string { return ''; } }
class Post {}

/**
 * @template TKey of array-key
 * @template TValue
 */
class Items
{
    /** @return TValue|null */
    public function first() { return null; }

    /**
     * @template TMapValue
     * @param callable(TValue, TKey): TMapValue $callback
     * @return $this
     * @phpstan-this-out static<TKey, TMapValue>
     */
    public function transform(callable $callback) { return $this; }
}

/**
 * @template TKey of array-key
 * @template TModel
 * @extends Items<TKey, TModel>
 */
class Models extends Items {}
"#;

#[test]
fn a_template_the_argument_binds_is_what_the_receiver_holds_afterwards() {
    check_view(
        &[
            BOX,
            r#"
//- /src/run.php
<?php
namespace App;

function run(string $next): void
{
    $box = new Box(1);
    $b@before@ox;
    $box->replace($next);
    $b@after@ox;
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("before the call", "before"),
            Query::variable_type("after the call", "after"),
        ],
        expect![[r#"
            before the call
            - Box<int>
            after the call
            - Box<string>
        "#]],
    );
}

/// `static` is the class the call was made on, and its `@extends` ladder carries the receiver's own
/// keys to the slot that names them.
#[test]
fn static_is_the_class_the_call_was_made_on_with_the_receivers_own_keys() {
    check_view(
        &[
            ITEMS,
            r#"
//- /src/run.php
<?php
namespace App;

/** @param Models<int, User> $models */
function run(Models $models): void
{
    $models->transform(fn ($user) => $user->name());
    $m@after@odels;
}
"#,
        ]
        .concat(),
        &[Query::variable_type("after transform", "after")],
        expect![[r#"
            after transform
            - Models<int, string>
        "#]],
    );
}

/// The receiver of the second call is what the first left, so its callback is handed a `Post` -- and
/// handing it back says so. Read off the receiver as it was built, the answer would be `User`.
#[test]
fn a_second_call_starts_from_what_the_first_left() {
    check_view(
        &[
            ITEMS,
            r#"
//- /src/run.php
<?php
namespace App;

/** @param Items<int, User> $items */
function run(Items $items): void
{
    $items->transform(fn (User $user): Post => new Post());
    $items->transform(fn ($post) => $post);
    $i@after@tems;
}
"#,
        ]
        .concat(),
        &[Query::variable_type("after both", "after")],
        expect![[r#"
            after both
            - Items<int, Post>
        "#]],
    );
}

/// A write after the call replaces what the call left, as it replaces any other claim about it.
#[test]
fn a_write_after_the_call_is_what_the_variable_holds() {
    check_view(
        &[
            BOX,
            r#"
//- /src/run.php
<?php
namespace App;

function run(string $next): void
{
    $box = new Box(1);
    $box->replace($next);
    $box = new Box(1.5);
    $b@after@ox;
}
"#,
        ]
        .concat(),
        &[Query::variable_type("after the write", "after")],
        expect![[r#"
            after the write
            - Box<float>
        "#]],
    );
}

/// A value read off the receiver is read off what the call left, not off what the receiver was
/// built as -- the chain and the bare variable answer the same.
#[test]
fn a_value_read_off_the_receiver_sees_what_the_call_left() {
    check_view(
        &[
            ITEMS,
            r#"
//- /src/run.php
<?php
namespace App;

/** @param Items<int, User> $items */
function run(Items $items): void
{
    $items->transform(fn (User $user): Post => new Post());
    $first = $items->first();
    $f@after@irst;
}
"#,
        ]
        .concat(),
        &[Query::variable_type("the first element", "after")],
        expect![[r#"
            the first element
            - ?Post
        "#]],
    );
}

/// A property keeps what a call made on it left, as a local does.
#[test]
fn a_property_receiver_holds_what_the_call_left() {
    check_view(
        &[
            ITEMS,
            r#"
//- /src/Holder.php
<?php
namespace App;

class Holder
{
    /** @var Items<int, User> */
    public Items $items;

    public function run(): void
    {
        $this->items->transform(fn (User $user): Post => new Post());
        $now = $this->items;
        $n@after@ow;
    }
}
"#,
        ]
        .concat(),
        &[Query::variable_type("the property afterwards", "after")],
        expect![[r#"
            the property afterwards
            - Items<int, Post>
        "#]],
    );
}

/// A receiver reached through a call is whatever that call handed back that time; the next call may
/// hand back another object, so nothing carries over to it.
#[test]
fn a_receiver_reached_through_a_call_carries_nothing_over() {
    check_view(
        &[
            ITEMS,
            r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /** @return Items<int, User> */
    public function items(): Items { return new Items(); }
}

function run(Repo $repo): void
{
    $repo->items()->transform(fn (User $user): Post => new Post());
    $again = $repo->items();
    $a@after@gain;
}
"#,
        ]
        .concat(),
        &[Query::variable_type("a second call", "after")],
        expect![[r#"
            a second call
            - Items<int, User>
        "#]],
    );
}

/// Symfony's `setParent()`: the out type names a template nothing binds, so nothing is stored and
/// the receiver keeps what it held rather than becoming a class nobody declares.
#[test]
fn an_out_type_naming_a_template_nothing_binds_leaves_the_receiver_alone() {
    check_view(
        r#"
//- /src/Node.php
<?php
namespace App;

class Root {}

/** @template TParent */
class Node
{
    /**
     * @template TNewParent
     * @psalm-this-out static<TNewParent>
     * @return $this
     */
    public function setParent(object $parent): static { return $this; }
}

/** @param Node<Root> $node */
function run(Node $node, object $other): void
{
    $node->setParent($other);
    $n@after@ode;
}
"#,
        &[Query::variable_type("after setParent", "after")],
        expect![[r#"
            after setParent
            - Node<Root>
        "#]],
    );
}
