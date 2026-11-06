//! `@param callable(A, B): C` reaches the lowered parameter as the callback's own parameters.
//!
//! PHP has no syntax for what a callback receives, so this tag is the only place it is ever
//! written, and the type built for the parameter's own slot keeps only the return -- a
//! `callable(A, B): C` is a `Generic { Callable, [C] }` there. The parameters answer a different
//! question and get a slot of their own.
//!
//! At this layer because the reader is three crates away, and a tag that does not reach the
//! lowering is inert without failing anything -- which reads as the feature not working rather than
//! as one written shape being missed. `check_hir` renders the slot as `calls(...)`, which it did not
//! before: a snapshot that does not show a field cannot test it.

use expect_test::expect;

use super::check_hir;

#[test]
fn a_documented_callback_carries_the_types_it_is_called_with() {
    check_hir(
        r#"
//- /src/app.php
<?php

/** @param callable(Item): void $each */
function one(callable $each): void {}

/** @param callable(string, int): bool $both */
function two(callable $both): void {}

/** The `Closure` spelling, and no return type written. */
/** @param \Closure(Item) $each */
function closure(\Closure $each): void {}

/** Nullable and parenthesized say whether one is passed, not what it receives. */
/** @param ?callable(Item): void $maybe */
function nullable(?callable $maybe): void {}

/** A bare callable says nothing about what it is called with. */
/** @param callable $bare */
function bare(callable $bare): void {}

/** Neither does one taking no arguments. */
/** @param callable(): void $none */
function none(callable $none): void {}
"#,
        expect![[r#"
            function one(callable $each calls(Item)): void @ 4:10-4:13  [doc]
            function two(callable $both calls(string, int)): void @ 7:10-7:13  [doc]
            function closure(\Closure $each calls(Item)): void @ 11:10-11:17  [doc]
            function nullable(?callable $maybe calls(Item)): void @ 15:10-15:18  [doc]
            function bare(callable $bare): void @ 19:10-19:14  [doc]
            function none(callable $none): void @ 23:10-23:14  [doc]
        "#]],
    );
}

#[test]
fn a_position_that_said_nothing_holds_the_place() {
    check_hir(
        r#"
//- /src/app.php
<?php

/** A position with no type keeps its place, so the ones beside it stay where they are. */
/** @param callable($a, Item $b): void $each */
function untyped(callable $each): void {}

/** A trailing `...` is dropped rather than modelled: nothing is said past the second. */
/** @param callable(string, Item...): void $rest */
function variadic(callable $rest): void {}

/** An optional parameter is still that position. */
/** @param callable(string, Item=): void $optional */
function optional(callable $optional): void {}

/** A by-reference one too. */
/** @param callable(string, Item &$out): void $byRef */
function byRef(callable $byRef): void {}
"#,
        expect![[r#"
            function untyped(callable $each calls(mixed, Item)): void @ 5:10-5:17  [doc]
            function variadic(callable $rest calls(string)): void @ 9:10-9:18  [doc]
            function optional(callable $optional calls(string, Item)): void @ 13:10-13:18  [doc]
            function byRef(callable $byRef calls(string, Item)): void @ 17:10-17:15  [doc]
        "#]],
    );
}

/// A template a container `@param` holds is an element, and says nothing here so that the call
/// site's container reading can: `mapped`'s `V` is what `array<K, V> $array` holds. One nothing holds
/// is kept as its name, which the view reads as "no element goes here" -- `array_reduce`'s
/// accumulator, `walk`'s `V`.
#[test]
fn a_template_a_container_holds_is_left_to_the_call_site() {
    check_hir(
        r#"
//- /src/app.php
<?php

/**
 * The shape the standard library actually writes: the callback's parameter is the array's element
 * type, bound by the argument, which nothing at this layer can bind.
 *
 * @template K of array-key
 * @template V
 * @template T
 * @param callable(V): T $callback
 * @param array<K, V> $array
 * @return array<K, T>
 */
function mapped(callable $callback, array $array): array {}

/**
 * A real class in the parameter and a template in the return: the half that works is kept.
 *
 * @template T
 * @param callable(Item): T $each
 * @return T
 */
function each(callable $each) {}

/**
 * Per position, so a template beside a class loses only its own place.
 *
 * @template V
 * @param callable(string, V, Item): void $walk
 */
function walk(callable $walk): void {}

/** A name that merely starts with a template's letters is not that template. */
/**
 * @template V
 * @param callable(Value): void $each
 */
function whole(callable $each): void {}
"#,
        expect![[r#"
            function mapped(callable $callback, array $array): array @ 14:10-14:16  [doc]
            function each(callable $each calls(Item)) @ 23:10-23:14  [doc]
            function walk(callable $walk calls(string, V, Item)): void @ 31:10-31:14  [doc]
            function whole(callable $each calls(Value)): void @ 38:10-38:15  [doc]
        "#]],
    );
}
