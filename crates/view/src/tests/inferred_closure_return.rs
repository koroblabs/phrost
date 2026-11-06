//! A block closure with no declared return type, answered from its `return` statements.
//!
//! The one closure shape that answered nothing: `function (): Row` declares its result and
//! `fn () => new Row()` *is* its result, and only `function () { return new Row(); }` fell between
//! them. The layer that owns it is this one -- what a `return` hands back is usually a class
//! declared in another file, so a body alone cannot settle it and a catalog is needed.
//!
//! Every fixture here runs through the harness's open-versus-closed dimension, which is what makes
//! this feature's central structural claim checkable rather than argued: the closure and its
//! invocation are in one body, so the statements are in the same HIR the question is asked against,
//! and only the *class* a return names has to come from elsewhere. Opening or closing that other
//! file must not change the answer.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_block_closures_return_is_read_from_its_body() {
    // The shape this was built for, in miniature: a factory closure over a captured service, a
    // local assigned from a method declared in another file, and a `return` of that local. On the
    // reference project this exact shape is one closure feeding eleven member accesses.
    check_view(
        r#"
//- /src/Query.php
<?php
namespace App;

class Query
{
    public function addExpression(string $expression): string {}
}

//- /src/Database.php
<?php
namespace App;

class Database
{
    public function select(string $table): Query {}
}

//- /src/Script.php
<?php
namespace App;

function run(Database $database): void
{
    $make = function () use ($database) {
        $query = $database->select('rows');
        return $query;
    };
    $dupes = $make();
    $du@invoked@pes;
    $ma@value@ke;
    $dupes->addExpr@member@ession('COUNT(*)');
}
"#,
        &[
            Query::variable_class("what invoking it gives", "invoked", "dupes"),
            Query::variable_type("the closure as a value", "value"),
            Query::goto("the member reached through it", "member"),
        ],
        expect![[r#"
            what invoking it gives
            - App\Query
            the closure as a value
            - Closure(): Query
            the member reached through it
            - method App\Query::addExpression @ src/Query.php 6:21-6:34
        "#]],
    );
}

#[test]
fn a_declared_return_type_wins_over_the_body() {
    // An inferred type must never displace a written one, however much better it looks. The body
    // returns a `Row`; the declaration says `Item`, which is what the author committed to and what
    // PHP enforces -- the mismatch is `incompatible-return`'s business, not this one's.
    check_view(
        r#"
//- /src/Kinds.php
<?php
namespace App;

class Row {}

class Item {}

//- /src/Use.php
<?php
namespace App;

function probe(): void
{
    $make = function (): Item {
        return new Row();
    };
    $got = $make();
    $g@got@ot;
}
"#,
        &[Query::variable_class(
            "declared, not inferred",
            "got",
            "got",
        )],
        expect![[r#"
            declared, not inferred
            - App\Item
        "#]],
    );
}

#[test]
fn every_return_naming_one_class_is_that_class_rather_than_a_union_of_copies() {
    // Three branches, one claim. `Row|Row|Row` names the right class and still reads as a choice
    // the code did not make, so the join folds it -- the same fold every other union in this layer
    // goes through.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row {}

//- /src/Use.php
<?php
namespace App;

function probe(int $n): void
{
    $make = function () use ($n) {
        if ($n === 1) {
            return new Row();
        }
        if ($n === 2) {
            return new Row();
        }
        return new Row();
    };
    $m@make@ake;
}
"#,
        &[Query::variable_type("folded", "make")],
        expect![[r#"
            folded
            - Closure(): Row
        "#]],
    );
}

#[test]
fn returns_of_different_classes_are_a_union() {
    // What the code actually said. Only what both branches agree on is reachable through the
    // result, which is the whole reason a union is kept rather than one operand picked.
    check_view(
        r#"
//- /src/Kinds.php
<?php
namespace App;

class Row {}

class Item {}

//- /src/Use.php
<?php
namespace App;

function probe(int $n): void
{
    $make = function () use ($n) {
        if ($n === 1) {
            return new Row();
        }
        return new Item();
    };
    $m@make@ake;
}
"#,
        &[Query::variable_type("both branches", "make")],
        expect![[r#"
            both branches
            - Closure(): Row|Item
        "#]],
    );
}

#[test]
fn one_return_that_resolves_to_nothing_makes_the_whole_closure_answer_nothing() {
    // Conservative or nothing. Half a union is a claim the code did not make: offering `Row`'s
    // members here would offer a call that is a fatal error on every run that took the other
    // branch. The value stays a callable, which is true and smaller.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row {}

//- /src/Use.php
<?php
namespace App;

function probe($anything): void
{
    $make = function () use ($anything) {
        if ($anything) {
            return new Row();
        }
        return $anything->whatever();
    };
    $m@make@ake;
}
"#,
        &[Query::variable_type("one branch unreadable", "make")],
        expect![[r#"
            one branch unreadable
            - callable
        "#]],
    );
}

#[test]
fn a_bare_return_contributes_null() {
    // `return;` hands back PHP's null, and dropping it to keep the answer tidy would be exactly the
    // nullable-suppression this project refuses.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row {}

//- /src/Use.php
<?php
namespace App;

function probe(int $n): void
{
    $make = function () use ($n) {
        if ($n === 1) {
            return new Row();
        }
        return;
    };
    $m@make@ake;
}
"#,
        &[Query::variable_type("nullable", "make")],
        expect![[r#"
            nullable
            - Closure(): ?Row
        "#]],
    );
}

#[test]
fn a_path_that_falls_off_the_end_contributes_null_too() {
    // The same fact written without the `return;`. A guard with no else is how most of these are
    // actually written, and PHP returns null down that path whether or not anybody typed the word.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row {}

//- /src/Use.php
<?php
namespace App;

function probe(int $n): void
{
    $make = function () use ($n) {
        if ($n === 1) {
            return new Row();
        }
    };
    $m@make@ake;
}
"#,
        &[Query::variable_type("implicitly nullable", "make")],
        expect![[r#"
            implicitly nullable
            - Closure(): ?Row
        "#]],
    );
}

#[test]
fn a_last_statement_that_returns_adds_no_null() {
    // The common case, and the one the null rule must not make worse: a body whose last statement
    // is a `return` cannot reach its own end, so there is no implicit null to add.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row {}

//- /src/Use.php
<?php
namespace App;

function probe(int $n): void
{
    $make = function () use ($n) {
        if ($n === 1) {
            return new Row();
        }
        return new Row();
    };
    $m@make@ake;
}
"#,
        &[Query::variable_type("no null anywhere", "make")],
        expect![[r#"
            no null anywhere
            - Closure(): Row
        "#]],
    );
}

#[test]
fn a_generator_is_not_its_returns() {
    // `yield` makes the closure produce a `Generator`, and a `return` inside one hands its value to
    // `Generator::getReturn()` rather than to the caller. Reading it as the result would type every
    // generator in a project as the thing it finishes with.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row {}

//- /src/Use.php
<?php
namespace App;

function probe(): void
{
    $rows = function () {
        yield new Row();
        return new Row();
    };
    $r@rows@ows;
}
"#,
        &[Query::variable_type("declined", "rows")],
        expect![[r#"
            declined
            - callable
        "#]],
    );
}

#[test]
fn a_closure_that_calls_itself_terminates_and_answers_nothing() {
    // A type defined in terms of itself. The inner ask is refused rather than followed, so the
    // recursion ends at the first re-entry instead of costing a full follow budget's worth of
    // branching -- and the outer union then has an operand that resolved to nothing, which is what
    // makes the whole answer nothing.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row {}

//- /src/Use.php
<?php
namespace App;

function probe(int $n): void
{
    $walk = function (int $depth) use (&$walk) {
        if ($depth === 0) {
            return new Row();
        }
        return $walk($depth - 1);
    };
    $w@walk@alk;
}
"#,
        &[Query::variable_type("self-referential", "walk")],
        expect![[r#"
            self-referential
            - callable
        "#]],
    );
}

#[test]
fn a_return_inside_a_nested_closure_belongs_to_the_nested_one() {
    // The rule every body walk in `phrost_body` states: a closure's body hangs off an expression,
    // so descending through statements alone is what keeps the two apart. Attributing the inner
    // `return` to the outer closure would type both as the wrong thing and neither as an error.
    check_view(
        r#"
//- /src/Kinds.php
<?php
namespace App;

class Row {}

class Item {}

//- /src/Use.php
<?php
namespace App;

function probe(): void
{
    $outer = function () {
        $inner = function () {
            return new Row();
        };
        return new Item();
    };
    $ou@outer@ter;
    $inner = function () {
        return new Row();
    };
    $in@inner@ner;
}
"#,
        &[
            Query::variable_type("the outer one", "outer"),
            Query::variable_type("the inner one", "inner"),
        ],
        expect![[r#"
            the outer one
            - Closure(): Item
            the inner one
            - Closure(): Row
        "#]],
    );
}

#[test]
fn a_closure_written_at_a_call_types_what_the_call_gives_back() {
    // The second reader of the same inference, and the one that reaches more code than a variable
    // does: `array_map` says its result is made of whatever the callback produced, and the callback
    // is the only place that is written. It already read an arrow function this way.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function id(): int {}
}

//- /stubs/standard.php
<?php

/**
 * @template K as array-key
 * @template V
 * @template U
 *
 * @param (callable(V): U)|null $callback
 * @param array<K, V> $array
 *
 * @return ($array is list<V> ? list<U> : array<K, U>)
 */
function array_map(?callable $callback, array $array): array {}

//- /src/Use.php
<?php
namespace App;

/**
 * @param list<Row> $rows
 */
function probe(array $rows): void
{
    $ids = array_map(function (Row $row) {
        return $row->id();
    }, $rows);
    $i@ids@ds;
}
"#,
        &[Query::variable_type("from the callback's body", "ids")],
        expect![[r#"
            from the callback's body
            - int[]
        "#]],
    );
}
