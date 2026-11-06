//! What a callback's parameter receives.
//!
//! `array_map(fn ($item) => ..., $items)` says nothing about `$item` — PHP has no syntax for it and
//! most code writes no docblock either. What the call does say is that the callback is applied to
//! the elements of `$items`, and every function that takes a callback beside a container works that
//! way.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_callbacks_parameter_is_the_element_of_the_container_beside_it() {
    check(
        r#"
//- /src/Walk.php
<?php
namespace App;

class Node
{
    public function label(): string { return ''; }
}

class Walk
{
    /** @return list<Node> */
    public function children(): array { return []; }

    public function run(): void
    {
        array_map(fn ($child) => $child->@here@, $this->children());
    }
}
"#,
        &[Query::completion("the callback's parameter", "here")],
        expect![[r#"
            the callback's parameter
            - label [method]  : string  writes `()`
            - accepting the first leaves `array_map(fn ($child) => $child->label, $this->children());`
        "#]],
    );
}

#[test]
fn the_container_is_found_whichever_side_it_was_written_on() {
    // `array_map` takes the callback first and `array_filter` second. Nothing here reads a signature
    // to know which, so the container is whichever argument has an element type.
    check(
        r#"
//- /src/Keep.php
<?php
namespace App;

class Node
{
    public function label(): string { return ''; }
}

class Keep
{
    /** @param list<Node> $nodes */
    public function run(array $nodes): void
    {
        array_filter($nodes, fn ($node) => $node->@here@);
    }
}
"#,
        &[Query::completion("the callback written second", "here")],
        expect![[r#"
            the callback written second
            - label [method]  : string  writes `()`
            - accepting the first leaves `array_filter($nodes, fn ($node) => $node->label);`
        "#]],
    );
}

#[test]
fn a_declared_container_parameter_narrows_to_what_the_call_holds() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Cell
{
    public function value(): int { return 0; }
}

class Rows
{
    /** @param list<list<Cell>> $rows */
    public function run(array $rows): void
    {
        array_map(fn (array $row) => $row[0]->@here@, $rows);
    }
}
"#,
        &[Query::completion("a bare array parameter", "here")],
        expect![[r#"
            a bare array parameter
            - value [method]  : int  writes `()`
            - accepting the first leaves `array_map(fn (array $row) => $row[0]->value, $rows);`
        "#]],
    );
}

#[test]
fn a_parameter_the_declaration_types_keeps_that_type() {
    // The call site is only consulted where the declaration said nothing useful. A written class is
    // the author's statement and beats anything inferred from the argument.
    check(
        r#"
//- /src/Typed.php
<?php
namespace App;

class Node
{
    public function label(): string { return ''; }
}

class Other
{
    public function elsewhere(): void {}
}

class Typed
{
    /** @param list<Other> $items */
    public function run(array $items): void
    {
        array_map(fn (Node $node) => $node->@here@, $items);
    }
}
"#,
        &[Query::completion("declared wins", "here")],
        expect![[r#"
            declared wins
            - label [method]  : string  writes `()`
            - accepting the first leaves `array_map(fn (Node $node) => $node->label, $items);`
        "#]],
    );
}

#[test]
fn a_later_parameter_is_not_the_element() {
    // The second parameter of a callback is a key or a second container, and nothing here can tell
    // those apart. Saying nothing is what keeps a key from being read as a value.
    check(
        r#"
//- /src/Keys.php
<?php
namespace App;

class Node
{
    public function label(): string { return ''; }
}

class Keys
{
    /** @param array<string, Node> $byId */
    public function run(array $byId): void
    {
        array_walk($byId, function ($node, $key) { $key->@here@; });
    }
}
"#,
        &[Query::completion("the key parameter", "here")],
        expect![[r#"
            the key parameter
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_callback_written_first_is_handed_one_element_of_each_container_after_it() {
    // `array_map` is the one shape in PHP that passes several containers, and it takes the callback
    // first: the parameters and the containers pair up in written order. A callback written *after*
    // a container is being told about that one container, which is why the pairing is read off
    // where the callback sits rather than applied everywhere.
    check(
        r#"
//- /src/Pairs.php
<?php
namespace App;

class Node
{
    public function label(): string { return ''; }
}

class Payment
{
    public function amount(): float { return 0.0; }
}

class Pairs
{
    /** @return list<Node> */
    public function nodes(): array { return []; }

    /** @return list<Payment> */
    public function payments(): array { return []; }

    public function run(): void
    {
        array_map(fn ($node, $payment) => $payment->@here@, $this->nodes(), $this->payments());
    }
}
"#,
        &[Query::completion("the second parameter", "here")],
        expect![[r#"
            the second parameter
            - amount [method]  : float  writes `()`
            - accepting the first leaves `array_map(fn ($node, $payment) => $payment->amount, $this->nodes(), $this->payments());`
        "#]],
    );
}

#[test]
fn a_container_that_says_nothing_about_its_elements_is_still_the_container() {
    // A declaration of `array` with nothing about its elements *is* the container, and reading
    // "has a known element type" as "is the container" walked past it to the next argument along --
    // which on the reference project was a list of floats, so the callback's first parameter
    // answered `float`. A wrong answer where there had been no answer.
    check(
        r#"
//- /src/Unknown.php
<?php
namespace App;

class Node
{
    public function label(): string { return ''; }
}

class Unknown
{
    public function slice(): array { return []; }

    /** @return list<Node> */
    public function nodes(): array { return []; }

    public function run(): void
    {
        array_map(fn ($first, $second) => $first->@here@, $this->slice(), $this->nodes());
    }
}
"#,
        &[Query::completion("the first parameter", "here")],
        expect![[r#"
            the first parameter
            - (nothing offered)
        "#]],
    );
}
