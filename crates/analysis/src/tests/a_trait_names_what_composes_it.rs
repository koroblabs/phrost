//! `@require-implements`, `@require-extends` and `@mixin` on a trait -- what every class composing it
//! is, and so what `$this` inside it is besides the trait.
//!
//! The tags are a claim about the composing class, so they are not merged into the trait's member
//! table: every composing class copies that table. `$this` holds the trait and what it names at once,
//! and everything read off `$this` -- a call's type, its declaration, its signature, a callback's
//! parameters -- reads the intersection.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_required_interface_answers_through_this() {
    check(
        r#"
//- /src/Months.php
<?php
namespace App;

interface Clock
{
    public function month(): int;
}

/** @require-implements Clock */
trait Months
{
    public function next(): int
    {
        $month = $this->month();
        $mo@type@nth;
        return $this->mon@goto@th();
    }
}
"#,
        &[
            Query::hover("the interface's return", "type"),
            Query::goto("the interface's declaration", "goto"),
        ],
        expect![[r#"
            the interface's return
            - code: int $month
            - note: assigned
            the interface's declaration
            - method App\Clock::month @ src/Months.php 6:21-6:26 (full 6:5-6:34)
        "#]],
    );
}

#[test]
fn a_callback_handed_to_a_borrowed_member_reads_its_signature() {
    // Laravel's `BuildsQueries` is `@mixin Builder` and hands closures to `$this->chunk()`: the
    // parameter's type comes from the member the call reached, and the signature shown is that one.
    check(
        r#"
//- /src/Walks.php
<?php
namespace App;

class Row
{
}

class Rows
{
    /** @param callable(Row): void $each */
    public function each(callable $each, int $size = 10): void {}
}

/** @mixin Rows */
trait Walks
{
    public function walk(): void
    {
        $this->each(function ($row) {
            $ro@row@w;
        }, @size@3);
    }
}
"#,
        &[
            Query::hover("the callback's parameter", "row"),
            Query::signature("the borrowed member's signature", "size"),
        ],
        expect![[r#"
            the callback's parameter
            - code: Row $row
            - note: parameter
            the borrowed member's signature
            - each(callable(...): void $each, int $size = 10)
                callable(...): void $each
              > int $size = 10
        "#]],
    );
}

#[test]
fn a_prefixed_require_extends_on_an_interface_reaches_the_class() {
    // On an interface the tag says every implementer extends the class, so a value typed as the
    // interface has the class's members -- read into the interface's table the way a mixin is.
    check(
        r#"
//- /src/Named.php
<?php
namespace App;

abstract class Base
{
    public function label(): string { return ''; }
}

/** @psalm-require-extends Base */
interface Named
{
}

class Runner
{
    public function run(Named $named): void
    {
        $named->@here@
    }
}
"#,
        &[Query::completion("the required class's members", "here")],
        expect![[r#"
            the required class's members
            - label [method]  : string  writes `();`
            - accepting the first leaves `$named->label`
        "#]],
    );
}
