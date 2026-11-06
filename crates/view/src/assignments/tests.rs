//! What an assignment to a typed property is judged to be.
//!
//! Every case here is as much about the silence as about the finding. A wrong "takes int, and this
//! is string" on somebody's property is the report that teaches a person to stop reading the list,
//! and each fixture that produces nothing names the reason it must.
//!
//! Run through `with_open_scopes`, so each case also asserts the verdict does not depend on which
//! files are open -- the declaration being judged against usually lives in another file.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;

use crate::IndexView;

/// Every incompatible assignment in the fixture's last file, whole.
fn check(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let path = spec
        .files()
        .last()
        .expect("the fixture declares a file")
        .path()
        .to_owned();
    let file = world.file_id(&path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let mut out = String::new();
        for found in view.incompatible_assignments() {
            let target = match &found.target {
                crate::assignments::AssignedTo::Property { class, property } => {
                    format!("{class}::${property}")
                }
                crate::assignments::AssignedTo::Annotated(name) => format!("${name}"),
            };
            let _ = writeln!(
                out,
                "{} {target} takes {}, and this is {}",
                world.render_range(file, found.range),
                found.declared,
                found.given
            );
        }
        if out.is_empty() {
            out.push_str("<nothing to report>\n");
        }
        out
    });

    expect.assert_eq(&rendered);
}

#[test]
fn a_typed_property_cannot_take_a_value_of_another_type() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Counter
{
    public int $count = 0;
}

function bump(Counter $counter): void
{
    $counter->count = 'bad';
}
"#,
        expect![[r#"
            11:23-11:28 Counter::$count takes int, and this is non-falsy-string
        "#]],
    );
}

#[test]
fn a_property_inherited_from_another_file_is_judged_and_named_by_the_receiver() {
    // The declaration is a parent's, in another file, which is the shape only an index can judge.
    // The class named is the one being written *through*, because that is the class PHP's own error
    // names and the one the reader has in front of them.
    check(
        r#"
//- /base.php
<?php
namespace App;

class Base
{
    protected int $total = 0;
}

//- /app.php
<?php
declare(strict_types=1);

namespace App;

class Invoice extends Base
{
    public function reset(): void
    {
        $this->total = 'nothing';
    }
}
"#,
        expect![[r#"
            10:24-10:33 App\Invoice::$total takes int, and this is non-falsy-string
        "#]],
    );
}

#[test]
fn a_static_property_is_judged_the_same_way() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Registry
{
    public static string $name = '';

    public static function set(int $id): void
    {
        self::$name = $id;
        Registry::$name = $id;
    }
}
"#,
        expect![[r#"
            10:23-10:26 Registry::$name takes string, and this is int
            11:27-11:30 Registry::$name takes string, and this is int
        "#]],
    );
}

#[test]
fn an_unrelated_class_cannot_be_stored_where_another_one_is_declared() {
    // Single inheritance is what makes the class provable: no class is below both, so no value can
    // satisfy the declaration. An *interface* in the same place is reported too, though a subclass
    // could implement it -- nothing declared makes a `Duration` one, and that is what PHPStan and
    // PHP's own `TypeError` go by.
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Money {}
class Duration {}

interface Renders {}

class Line
{
    public Money $price;
    public Renders $shown;

    public function fill(Duration $wrong): void
    {
        $this->price = $wrong;
        $this->shown = $wrong;
    }
}
"#,
        expect![[r#"
            16:24-16:30 Line::$price takes \Money, and this is Duration
            17:24-17:30 Line::$shown takes \Renders, and this is Duration
        "#]],
    );
}

#[test]
fn a_property_with_no_declared_type_takes_anything() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Bag
{
    public $anything;

    public function fill(int $n): void
    {
        $this->anything = $n;
    }
}
"#,
        expect![[r#"
            <nothing to report>
        "#]],
    );
}

#[test]
fn int_into_float_is_the_one_widening_strict_types_still_performs() {
    // A conversion PHP does under every setting, so it is right even in the strictest file. The
    // other direction loses the fraction and is refused, which is the second line of the fixture.
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Reading
{
    public float $ratio = 0.0;
    public int $count = 0;

    public function fill(int $whole, float $fraction): void
    {
        $this->ratio = $whole;
        $this->count = $fraction;
    }
}
"#,
        expect![[r#"
            12:24-12:33 Reading::$count takes int, and this is float
        "#]],
    );
}

#[test]
fn the_files_own_strict_types_decides_a_conversion() {
    // The same write as the first test, in a file that never declared the directive: PHP converts
    // the string and nothing is wrong. The directive is read off the file the assignment is written
    // in, which is the thing most likely to end up wired to the wrong file.
    check(
        r#"
//- /app.php
<?php
class Counter
{
    public int $count = 0;
}

function bump(Counter $counter, string $written): void
{
    $counter->count = $written;
}
"#,
        expect![[r#"
            <nothing to report>
        "#]],
    );
}

#[test]
fn a_docblock_property_is_left_alone_because_set_answers_it() {
    // `@property` describes what `__set` does with a name the class never declared. The runtime
    // enforces nothing, so a rejection here would be an argument with a comment.
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

/**
 * @property int $virtual
 */
class Magic
{
    public function __get(string $name): mixed { return null; }

    public function __set(string $name, mixed $value): void {}

    public function fill(): void
    {
        $this->virtual = 'text';
    }
}
"#,
        expect![[r#"
            <nothing to report>
        "#]],
    );
}

#[test]
fn a_property_this_scope_cannot_write_directly_is_left_to_the_other_checks() {
    // Out of reach, so either `__set` takes the write -- and the declared type says nothing about
    // it -- or the mistake is the access rather than the value.
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Vault
{
    private int $secret = 0;
}

function poke(Vault $vault): void
{
    $vault->secret = 'text';
}
"#,
        expect![[r#"
            <nothing to report>
        "#]],
    );
}

#[test]
fn a_receiver_that_may_be_null_belongs_to_the_null_check() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Counter
{
    public int $count = 0;
}

function bump(?Counter $counter): void
{
    $counter->count = 'bad';
}
"#,
        expect![[r#"
            <nothing to report>
        "#]],
    );
}

#[test]
fn a_receiver_with_no_type_says_nothing_about_what_it_holds() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

function bump($whatever): void
{
    $whatever->count = 'bad';
}
"#,
        expect![[r#"
            <nothing to report>
        "#]],
    );
}

#[test]
fn a_promoted_constructor_property_is_a_property_like_any_other() {
    // The parameter is the declaration, and PHP enforces it on a later write exactly as it would a
    // written one.
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Order
{
    public function __construct(public int $quantity) {}

    public function clear(): void
    {
        $this->quantity = 'none';
    }
}
"#,
        expect![[r#"
            10:27-10:33 Order::$quantity takes int, and this is non-falsy-string
        "#]],
    );
}

#[test]
fn a_union_with_one_branch_that_cannot_fit_is_reported() {
    // Decided rather than defaulted, and it is what most of the real findings look like: a
    // declaration wider than the value it usually carries, stored somewhere narrower. Both halves
    // have the same remedy -- narrow before assigning -- and no rule tells the noisy case from the
    // valuable one, so suppressing either would take the other with it.
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Store
{
    private string $raw = '';
    private ?string $path = NULL;

    public function fill(): void
    {
        $this->raw = $this->read();
        $this->path = $this->find();
    }

    private function read(): string|array
    {
        return '';
    }

    private function find(): string|false
    {
        return FALSE;
    }
}
"#,
        expect![[r#"
            11:22-11:35 Store::$raw takes string, and this is string|array
            12:23-12:36 Store::$path takes ?string, and this is string|false
        "#]],
    );
}

#[test]
fn an_annotated_local_is_judged_against_what_it_was_annotated_with() {
    // Both sides of this are written in the file: `@var File` is an index into its own arena, and
    // so is whatever the value resolves through. Neither can be interned by a reading view -- every
    // `TypeTable` constructor needs `&mut` -- so the judgement describes both instead.
    //
    // This test was written the other way round, pinning the hole, and it is what said the hole was
    // gone.
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class File {}

function open(): void
{
    /** @var File $file */
    $file = 'bad';
}
"#,
        expect![[r#"
            9:13-9:18 $file takes File, and this is non-falsy-string
        "#]],
    );
}

#[test]
fn a_cast_is_read_against_an_annotation_too() {
    // Weak mode, where the two rules about what was *written* are the only thing that can speak:
    // the judgement itself accepts every scalar where any scalar is declared. The same line written
    // into a typed property has always been reported, and this site reaches the judgement by
    // another entry point -- see `IndexView::fit_written_value` -- so it was blind to both.
    check(
        r#"
//- /app.php
<?php

function open(int $n): void
{
    /** @var string $id */
    $id = (int) $n;
}
"#,
        expect![[r#"
            6:11-6:19 $id takes string, and this is int
        "#]],
    );
}

#[test]
fn a_literal_no_conversion_can_save_is_read_against_an_annotation_too() {
    // The other of the two, and the one that was already there to be missed: `'nope'` is a
    // `TypeError` where an `int` is declared, on every run and whatever `strict_types` says.
    check(
        r#"
//- /app.php
<?php

function open(): void
{
    /** @var int $count */
    $count = 'nope';
}
"#,
        expect![[r#"
            6:14-6:20 $count takes int, and this is non-falsy-string
        "#]],
    );
}

#[test]
fn an_annotation_that_takes_a_null_off_a_value_is_why_the_tag_is_written() {
    // The policy the two rules above pass through rather than around, and the reason it is a
    // policy: a `@var` is an assertion and PHP enforces nothing, so the commonest tag in real code
    // is one narrowing a `?T` the signature could not promise. Reporting it would be arguing with
    // the author about the one thing the annotation exists to say -- measured at 30 of 35 findings
    // on a real project.
    check(
        r#"
//- /app.php
<?php

class Node {}

function load(): ?Node
{
    return NULL;
}

function open(): void
{
    /** @var Node $node */
    $node = load();
}
"#,
        expect![[r#"
            <nothing to report>
        "#]],
    );
}

#[test]
fn an_annotation_that_names_one_branch_of_a_union_is_an_assertion_too() {
    // The null rule above, for every other branch: `/** @var int $pos */` over `strpos()`'s
    // `int|false` is the author saying which of the two this one is, and so is `object|false` over
    // Drupal's `bool|object`. Measured at 76 findings across three corpora, every one of them a
    // narrowing like these. An annotation no branch of the value can be still contradicts it.
    check(
        r#"
//- /app.php
<?php

class Node {}

/** @return int|false */
function position() { return 1; }

/** @return bool|Node */
function claim() { return false; }

function open(): void
{
    /** @var int $pos */
    $pos = position();
    /** @var Node|false $item */
    $item = claim();
    /** @var Node $node */
    $node = position();
}
"#,
        expect![[r#"
            18:13-18:23 $node takes Node, and this is int|false
        "#]],
    );
}

#[test]
fn a_target_the_body_layer_declines_to_name_produces_nothing() {
    // A computed member names no property and an append names no key. `assignment_sites` already
    // refuses both; this is the test that says the projection inherits that refusal.
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Counter
{
    public int $count = 0;
}

function bump(Counter $counter, string $field, array $rows): void
{
    $counter->$field = 'bad';
    $rows[] = 'bad';
    $counter->count += 'bad';
}
"#,
        expect![[r#"
            <nothing to report>
        "#]],
    );
}
