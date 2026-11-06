//! `$a ?? $b`, and the ternary — a value the code wrote two of.
//!
//! Both operands are kept and reduced together, because that is what the value is: one of them, and
//! nobody here knows which. A member both agree on is reached through it plainly; a member only one
//! of them has is reached too, with the operand's name beside it -- the same rule a written `A|B`
//! follows, see [`composites`](super::composites).
//!
//! A join is the shape that decided that rule, in fact. `PathRoot::EitherOf` reads every operand at
//! the joining expression's own offset, so the union it builds can be *wider* than the value really
//! is -- which is why nothing here may be reported, and why hiding a row rather than marking it
//! bought nothing.
//!
//! Read at all, which is the change these pin: a `??` used to resolve to nothing, and `?? ''` is
//! most of how a default is written.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_coalesce_reaches_what_both_operands_share() {
    check(
        r#"
//- /app.php
<?php
class Worker
{
    public function doWork(): void {}
    public function onlyWorker(): void {}
}

class Fallback
{
    public function doWork(): void {}
    public function onlyFallback(): void {}
}

function maybeWorker(): ?Worker {}

function run(): void
{
    $w = maybeWorker() ?? new Fallback();
    $@here@w;
    $w->$0
}
"#,
        &[
            Query::hover("what the coalesce holds", "here"),
            Query::completion("through a coalesce", "0"),
        ],
        expect![[r#"
            what the coalesce holds
            - code: Worker|Fallback $w
            - note: assigned
            through a coalesce
            - doWork [method]  : void  writes `();`
            - onlyWorker [method]  : void — Worker only  writes `();`
            - onlyFallback [method]  : void — Fallback only  writes `();`
            - accepting the first leaves `$w->doWork`
        "#]],
    );
}

#[test]
fn the_null_the_operator_ruled_out_does_not_survive_it() {
    // `?? ''` is written to rule the null out. A left operand whose null came back would make the
    // one thing the author handled read as still possible.
    check(
        r#"
//- /app.php
<?php
class Row
{
    public function id(): int {}
}

function maybeRow(): ?Row {}

function run(): void
{
    $row = maybeRow() ?? new Row();
    $@here@row;
}
"#,
        &[Query::hover("after a coalesce", "here")],
        expect![[r#"
            after a coalesce
            - code: Row $row
            - note: assigned
        "#]],
    );
}

#[test]
fn a_ternary_reaches_what_both_arms_share() {
    check(
        r#"
//- /app.php
<?php
class Foo {
    public function shared(): void {}
    public function fooMethod(): void {}
}
class Bar {
    public function shared(): void {}
    public function barMethod(): void {}
}

function test(bool $cond): void
{
    $x = $cond ? new Foo() : new Bar();
    $x->$0
}
"#,
        &[Query::completion("through a ternary", "0")],
        expect![[r#"
            through a ternary
            - shared [method]  : void  writes `();`
            - fooMethod [method]  : void — Foo only  writes `();`
            - barMethod [method]  : void — Bar only  writes `();`
            - accepting the first leaves `$x->shared`
        "#]],
    );
}

#[test]
fn a_ternary_with_a_null_arm_is_a_nullable() {
    check(
        r#"
//- /app.php
<?php
class Row
{
    public function id(): int {}
}

function run(bool $cond): void
{
    $row = $cond ? new Row() : NULL;
    $@here@row;
}
"#,
        &[Query::hover("one arm is null", "here")],
        expect![[r#"
            one arm is null
            - code: ?Row $row
            - note: assigned
        "#]],
    );
}

#[test]
fn a_default_read_out_of_a_shape_keeps_the_shapes_value() {
    // `$prices[$key] ?? null` then a null guard is how a shape is read defensively, and each half of
    // it used to lose the type: the coalesce resolved to nothing at all.
    check(
        r#"
//- /app.php
<?php
class Decimal {
    public function toString(): string { return ''; }
}

class PriceService {
    /** @return array{normalPrice: Decimal, memberPrice: Decimal} */
    private function getPrices(): array { return []; }

    public function format(string $key): string
    {
        $prices = $this->getPrices();
        $price = $prices[$key] ?? null;
        if ($price === null) {
            return '';
        }
        $price->$0
    }
}
"#,
        &[Query::completion("a guarded default out of a shape", "0")],
        expect![[r#"
            a guarded default out of a shape
            - toString [method]  : string  writes `();`
            - accepting the first leaves `$price->toString`
        "#]],
    );
}
