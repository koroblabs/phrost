//! Reaching into what a value holds.
//!
//! `$rows[0]->` and `$row['app']->` are written everywhere in PHP, and until the receiver scanner
//! could read a subscript both offered nothing at all — the type was known, and the characters
//! before the cursor were what stood in the way.
//!
//! The two halves are separate answers. A shape says what *this* key holds, so `array{app:
//! Application, count: int}` indexed by `app` is an `Application`; everything else holds the same
//! thing at every slot, so the key picked one and the element type is the answer.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn an_element_of_a_list_is_what_the_list_holds() {
    check(
        r#"
//- /src/Box.php
<?php
namespace App;

class Item
{
    public function label(): string { return ''; }
}

class Box
{
    /** @param list<Item> $items */
    public function run(array $items): void
    {
        $items[0]->@here@
    }
}
"#,
        &[Query::completion("element of a list", "here")],
        expect![[r#"
            element of a list
            - label [method]  : string  writes `();`
            - accepting the first leaves `$items[0]->label`
        "#]],
    );
}

#[test]
fn a_shape_answers_the_key_it_was_given() {
    check(
        r#"
//- /src/Row.php
<?php
namespace App;

class Application
{
    public function submit(): void {}
}

class Row
{
    /** @param array{app: Application, count: int} $row */
    public function run(array $row): void
    {
        $row['app']->@here@
    }
}
"#,
        &[Query::completion("the shape's own key", "here")],
        expect![[r#"
            the shape's own key
            - submit [method]  : void  writes `();`
            - accepting the first leaves `$row['app']->submit`
        "#]],
    );
}

#[test]
fn a_key_the_shape_does_not_name_reaches_nothing() {
    // The honest answer, and the fact a report could be built on later: the shape is the whole
    // statement of what is in there, so a key it does not name is a key that is not in there.
    check(
        r#"
//- /src/Row.php
<?php
namespace App;

class Application
{
    public function submit(): void {}
}

class Row
{
    /** @param array{app: Application} $row */
    public function run(array $row): void
    {
        $row['typo']->@here@
    }
}
"#,
        &[Query::completion("a key that is not there", "here")],
        expect![[r#"
            a key that is not there
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_subscript_applies_to_what_the_step_before_it_returned() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Item
{
    public function label(): string { return ''; }
}

class Repo
{
    /** @return list<Item> */
    public function all(): array { return []; }
}

class Uses
{
    public function run(Repo $repo): void
    {
        $repo->all()[0]->@here@
    }
}
"#,
        &[Query::completion("element of what a call returned", "here")],
        expect![[r#"
            element of what a call returned
            - label [method]  : string  writes `();`
            - accepting the first leaves `$repo->all()[0]->label`
        "#]],
    );
}

#[test]
fn subscripts_stack() {
    check(
        r#"
//- /src/Grid.php
<?php
namespace App;

class Cell
{
    public function value(): int { return 0; }
}

class Grid
{
    /** @param list<list<Cell>> $rows */
    public function run(array $rows): void
    {
        $rows[0][1]->@here@
    }
}
"#,
        &[Query::completion("an element of an element", "here")],
        expect![[r#"
            an element of an element
            - value [method]  : int  writes `();`
            - accepting the first leaves `$rows[0][1]->value`
        "#]],
    );
}

#[test]
fn a_computed_key_reaches_the_element_and_not_a_field() {
    // `$row[$key]` names a slot nothing here can identify. On a shape that means no single answer,
    // and on a keyed array it means what every slot holds — which is the same answer as any index.
    check(
        r#"
//- /src/Map.php
<?php
namespace App;

class Item
{
    public function label(): string { return ''; }
}

class Map
{
    /** @param array<string, Item> $byId */
    public function run(array $byId, string $id): void
    {
        $byId[$id]->@here@
    }
}
"#,
        &[Query::completion("a variable key", "here")],
        expect![[r#"
            a variable key
            - label [method]  : string  writes `();`
            - accepting the first leaves `$byId[$id]->label`
        "#]],
    );
}

#[test]
fn an_offset_read_off_a_string_is_a_string() {
    // PHP's one container whose element is the same type as the container. Worth its own answer
    // because the alternative is silence: a string has no element type for the general rule to find.
    check(
        r#"
//- /src/Word.php
<?php
namespace App;

class Word
{
    public function run(string $word): void
    {
        $first = $word[0];
        $fir@here@st;
    }
}
"#,
        &[Query::hover("an offset of a string", "here")],
        expect![[r#"
            an offset of a string
            - code: string $first
            - note: assigned
        "#]],
    );
}

#[test]
fn a_computed_key_on_a_shape_answers_when_every_field_agrees() {
    // The key reaches every slot at once, so the shape has one answer exactly when its fields do.
    check(
        r#"
//- /src/Prices.php
<?php
namespace App;

class Money
{
    public function format(): string { return ''; }
}

class Prices
{
    /** @param array{normal: Money, member: Money} $prices */
    public function run(array $prices, string $which): void
    {
        $prices[$which]->@here@
    }
}
"#,
        &[Query::completion("every field is a Money", "here")],
        expect![[r#"
            every field is a Money
            - format [method]  : string  writes `();`
            - accepting the first leaves `$prices[$which]->format`
        "#]],
    );
}

#[test]
fn a_computed_key_on_a_mixed_shape_offers_both_fields_and_says_whose() {
    // `array{pen: Pen, pencil: Pencil}` read at an unknown key is one or the other, so both fields'
    // members are offered and each says which field it came from. Nothing here is unmarked, which is
    // the claim: the two share no member, so no row can be reached whichever key it turns out to be.
    check(
        r#"
//- /src/Kit.php
<?php
namespace App;

class Pen
{
    public function write(): void {}
}

class Pencil
{
    public function sketch(): void {}
}

class Kit
{
    /** @param array{pen: Pen, pencil: Pencil} $kit */
    public function run(array $kit, string $which): void
    {
        $kit[$which]->@here@
    }
}
"#,
        &[Query::completion("two different fields", "here")],
        expect![[r#"
            two different fields
            - write [method]  : void — Pen only  writes `();`
            - sketch [method]  : void — Pencil only  writes `();`
            - accepting the first leaves `$kit[$which]->write`
        "#]],
    );
}
