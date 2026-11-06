//! What a guard that cannot fall through proves about the code after it.
//!
//! The dominant shape in PHP: `if (!$x instanceof Foo) { ... }` followed by everything the function
//! actually does. What matters is only whether the block can be got past, and PHP has four ways of
//! saying it cannot — three of them syntax and one of them a declaration in another file.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_guard_ending_in_exit_narrows_what_follows() {
    check(
        r#"
//- /src/Run.php
<?php
namespace App;

class Foo
{
    public function fooMethod(): void {}
}

class Bar
{
    public function barMethod(): void {}
}

class Run
{
    public function test(Foo|Bar $x): void
    {
        if (!$x instanceof Foo) {
            exit;
        }

        $x->@here@
    }
}
"#,
        &[Query::completion("past an exit", "here")],
        expect![[r#"
            past an exit
            - fooMethod [method]  : void  writes `();`
            - accepting the first leaves `$x->fooMethod`
        "#]],
    );
}

#[test]
fn a_guard_ending_in_a_call_that_never_returns_narrows_what_follows() {
    // The one shape syntax cannot settle: `fail()` is written exactly like a call that comes back,
    // and only its declaration -- in whatever file that is -- says it does not.
    check(
        r#"
//- /src/Aborter.php
<?php
namespace App;

class Aborter
{
    public function fail(): never
    {
        throw new \RuntimeException();
    }
}
//- /src/Run.php
<?php
namespace App;

class Foo
{
    public function fooMethod(): void {}
}

class Bar
{
    public function barMethod(): void {}
}

class Run
{
    public function test(Foo|Bar $x, Aborter $aborter): void
    {
        if (!$x instanceof Foo) {
            $aborter->fail();
        }

        $x->@here@
    }
}
"#,
        &[Query::completion("past a never-returning call", "here")],
        expect![[r#"
            past a never-returning call
            - fooMethod [method]  : void  writes `();`
            - accepting the first leaves `$x->fooMethod`
        "#]],
    );
}

#[test]
fn a_guard_whose_block_can_be_got_past_proves_nothing() {
    // A call that returns leaves the block reachable, so the value below is still either class.
    // What proves it is the marks: both operands' own members are offered and *both* are labelled,
    // where a value narrowed to `Foo` would offer `fooMethod` with nothing beside it.
    check(
        r#"
//- /src/Run.php
<?php
namespace App;

class Foo
{
    public function fooMethod(): void {}
}

class Bar
{
    public function barMethod(): void {}
}

class Run
{
    public function log(): void {}

    public function test(Foo|Bar $x): void
    {
        if (!$x instanceof Foo) {
            $this->log();
        }

        $x->@here@
    }
}
"#,
        &[Query::completion("past a call that returns", "here")],
        expect![[r#"
            past a call that returns
            - fooMethod [method]  : void — Foo only  writes `();`
            - barMethod [method]  : void — Bar only  writes `();`
            - accepting the first leaves `$x->fooMethod`
        "#]],
    );
}

#[test]
fn a_guard_over_a_property_narrows_the_receiver_written_after_it() {
    // The completion scanner starts from characters, not from an expression, so a guard is not
    // something it can see. A property is where that matters: a local's guard is already folded in
    // where the local is resolved, and a property has no such place.
    check(
        r#"
//- /src/Trip.php
<?php
namespace App;

interface Timeline
{
    public function items(): void;
}

class Trip implements Timeline
{
    public function items(): void {}

    public function customer(): string { return ''; }
}

class Booking
{
    private Timeline $timeline;

    public function run(): void
    {
        if ($this->timeline instanceof Trip) {
            $this->timeline->@here@
        }
    }
}
"#,
        &[Query::completion("a guarded property", "here")],
        expect![[r#"
            a guarded property
            - customer [method]  : string  writes `();`
            - items [method]  : void  writes `();`
            - accepting the first leaves `$this->timeline->customer`
        "#]],
    );
}

#[test]
fn a_guard_over_one_property_says_nothing_about_another() {
    check(
        r#"
//- /src/Pair.php
<?php
namespace App;

interface Timeline
{
    public function items(): void;
}

class Trip implements Timeline
{
    public function items(): void {}

    public function customer(): string { return ''; }
}

class Pair
{
    private Timeline $first;

    private Timeline $second;

    public function run(): void
    {
        if ($this->first instanceof Trip) {
            $this->second->@here@
        }
    }
}
"#,
        &[Query::completion("the other property", "here")],
        expect![[r#"
            the other property
            - items [method]  : void  writes `();`
            - accepting the first leaves `$this->second->items`
        "#]],
    );
}

#[test]
fn a_type_predicate_leaves_the_operand_it_ruled_out() {
    // How a PHP function that accepts either is written: take `Foo|string`, deal with the string,
    // and the rest of the body has a `Foo`. Nothing names a class anywhere in it.
    check(
        r#"
//- /src/Accept.php
<?php
namespace App;

class Foo
{
    public function fooMethod(): void {}
}

class Accept
{
    public function run(Foo|string $x): void
    {
        if (is_string($x)) {
            return;
        }

        $x->@here@
    }
}
"#,
        &[Query::completion("past the string", "here")],
        expect![[r#"
            past the string
            - fooMethod [method]  : void  writes `();`
            - accepting the first leaves `$x->fooMethod`
        "#]],
    );
}

#[test]
fn a_type_predicate_holding_keeps_the_operand_it_named() {
    check(
        r#"
//- /src/Either.php
<?php
namespace App;

class Row
{
    public function label(): string { return ''; }
}

class Either
{
    /** @param Row|list<Row> $value */
    public function run(mixed $value): void
    {
        if (is_array($value)) {
            foreach ($value as $one) {
                $one->@here@
            }
        }
    }
}
"#,
        &[Query::completion("inside the array branch", "here")],
        expect![[r#"
            inside the array branch
            - label [method]  : string  writes `();`
            - accepting the first leaves `$one->label`
        "#]],
    );
}

#[test]
fn a_predicate_ruling_out_one_of_three_operands_leaves_the_other_two() {
    // `Foo|Bar|string` minus the string is still two classes, and two classes is an answer: what
    // both of them have. The scalar going is what makes it one -- a union with a `string` in it can
    // be reached with `->` at all only by accident.
    check(
        r#"
//- /src/Three.php
<?php
namespace App;

class Foo
{
    public function shared(): void {}

    public function onlyFoo(): void {}
}

class Bar
{
    public function shared(): void {}
}

class Three
{
    public function run(Foo|Bar|string $x): void
    {
        if (is_string($x)) {
            return;
        }

        $x->@here@
    }
}
"#,
        &[Query::completion("two classes left", "here")],
        expect![[r#"
            two classes left
            - shared [method]  : void  writes `();`
            - onlyFoo [method]  : void — Foo only  writes `();`
            - accepting the first leaves `$x->shared`
        "#]],
    );
}

#[test]
fn an_instanceof_guard_that_leaves_names_the_operand_it_did_not_rule_out() {
    // The other side of an `instanceof`. It names no class of its own -- but the declaration listed
    // the alternatives, and taking one away leaves one.
    check(
        r#"
//- /src/Pick.php
<?php
namespace App;

class Foo
{
    public function fooOnly(): void {}
}

class Bar
{
    public function barOnly(): void {}
}

class Pick
{
    public function run(Foo|Bar $value): void
    {
        if ($value instanceof Foo) {
            return;
        }

        $value->@here@
    }
}
"#,
        &[Query::completion("everything but Foo", "here")],
        expect![[r#"
            everything but Foo
            - barOnly [method]  : void  writes `();`
            - accepting the first leaves `$value->barOnly`
        "#]],
    );
}

#[test]
fn ruling_a_class_out_takes_its_subclasses_with_it() {
    // A `Special` is a `Foo`, so a guard that ruled `Foo` out ruled it out too. Keeping it would
    // offer members of a class the value cannot be.
    check(
        r#"
//- /src/Sub.php
<?php
namespace App;

class Foo
{
    public function fooOnly(): void {}
}

class Special extends Foo
{
    public function specialOnly(): void {}
}

class Bar
{
    public function barOnly(): void {}
}

class Sub
{
    public function run(Special|Bar $value): void
    {
        if ($value instanceof Foo) {
            return;
        }

        $value->@here@
    }
}
"#,
        &[Query::completion("Special went with Foo", "here")],
        expect![[r#"
            Special went with Foo
            - barOnly [method]  : void  writes `();`
            - accepting the first leaves `$value->barOnly`
        "#]],
    );
}

#[test]
fn an_or_of_instanceofs_keeps_every_class_it_named() {
    check(
        r#"
//- /src/Either.php
<?php
namespace App;

class Alpha
{
    public function shared(): void {}

    public function onlyAlpha(): void {}
}

class Beta
{
    public function shared(): void {}
}

class Gamma
{
    public function onlyGamma(): void {}
}

class Either
{
    public function run(Alpha|Beta|Gamma $x): void
    {
        if ($x instanceof Alpha || $x instanceof Beta) {
            $x->@here@
        }
    }
}
"#,
        &[Query::completion("one of two", "here")],
        expect![[r#"
            one of two
            - shared [method]  : void  writes `();`
            - onlyAlpha [method]  : void — Alpha only  writes `();`
            - accepting the first leaves `$x->shared`
        "#]],
    );
}

#[test]
fn the_else_of_an_or_rules_out_every_class_it_named() {
    // `!(A || B)` is `!A && !B`, so both claims hold. Composed by nesting rather than ranked: taking
    // whichever was stronger would rule out one class and forget the other.
    check(
        r#"
//- /src/Rest.php
<?php
namespace App;

class Alpha
{
    public function onlyAlpha(): void {}
}

class Beta
{
    public function onlyBeta(): void {}
}

class Gamma
{
    public function onlyGamma(): void {}
}

class Rest
{
    public function run(Alpha|Beta|Gamma $x): void
    {
        if ($x instanceof Alpha || $x instanceof Beta) {
            return;
        }

        $x->@here@
    }
}
"#,
        &[Query::completion("neither of the two", "here")],
        expect![[r#"
            neither of the two
            - onlyGamma [method]  : void  writes `();`
            - accepting the first leaves `$x->onlyGamma`
        "#]],
    );
}

#[test]
fn a_guard_ending_in_a_free_function_that_never_returns_narrows_what_follows() {
    // The same question asked of the other table. A free function's row carries an index into its
    // file's declared symbols rather than a name, so the name a call writes has to be derived from
    // the fully-qualified one -- and the filter in front of this lookup is keyed on exactly that
    // short name. Getting it wrong here is silent: the guard simply stops narrowing.
    check(
        r#"
//- /src/abort.php
<?php
namespace App;

function abort_now(): never
{
    throw new \RuntimeException();
}
//- /src/Run.php
<?php
namespace App;

class Foo
{
    public function fooMethod(): void {}
}

class Bar
{
    public function barMethod(): void {}
}

class Run
{
    public function test(Foo|Bar $x): void
    {
        if (!$x instanceof Foo) {
            abort_now();
        }

        $x->@here@
    }
}
"#,
        &[Query::completion("past a never-returning function", "here")],
        expect![[r#"
            past a never-returning function
            - fooMethod [method]  : void  writes `();`
            - accepting the first leaves `$x->fooMethod`
        "#]],
    );
}
