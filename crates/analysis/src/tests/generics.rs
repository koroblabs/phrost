//! What a subclass filled in for a container's template parameters.
//!
//! PHP has no generics, so this is written in two halves: `@template T` on the container and
//! `@extends Bar<Baz>` on whoever extended it. A member typed `T` says nothing on its own — it is a
//! name resolving to no class — and what it means depends on which class it was reached through.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_member_typed_by_a_template_becomes_what_the_subclass_supplied() {
    check(
        r#"
//- /src/Bag.php
<?php
namespace App;

class Item
{
    public function label(): string { return ''; }
}

/** @template T */
class Bag
{
    /** @return T */
    public function first() {}
}

/** @extends Bag<Item> */
class Items extends Bag {}

class Runner
{
    public function run(Items $items): void
    {
        $items->first()->@here@
    }
}
"#,
        &[Query::completion("through one hop", "here")],
        expect![[r#"
            through one hop
            - label [method]  : string  writes `();`
            - accepting the first leaves `$items->first()->label`
        "#]],
    );
}

#[test]
fn an_argument_is_carried_through_a_class_that_only_passes_it_on() {
    // `Middle<T> extends Base<T>` supplies nothing of its own: what `Base` gets is whatever the
    // class below gave `Middle`.
    check(
        r#"
//- /src/Chain.php
<?php
namespace App;

class Item
{
    public function label(): string { return ''; }
}

/** @template T */
class Base
{
    /** @return T */
    public function first() {}
}

/**
 * @template T
 * @extends Base<T>
 */
class Middle extends Base {}

/** @extends Middle<Item> */
class Leaf extends Middle {}

class Runner
{
    public function run(Leaf $leaf): void
    {
        $leaf->first()->@here@
    }
}
"#,
        &[Query::completion("through two hops", "here")],
        expect![[r#"
            through two hops
            - label [method]  : string  writes `();`
            - accepting the first leaves `$leaf->first()->label`
        "#]],
    );
}

#[test]
fn iterating_a_container_yields_what_it_was_given() {
    check(
        r#"
//- /src/Iterate.php
<?php
namespace App;

class Row
{
    public function value(): int { return 0; }
}

/**
 * @template T
 * @extends \IteratorAggregate<int, T>
 */
interface Collection extends \IteratorAggregate {}

/** @extends Collection<Row> */
interface Rows extends Collection {}

class Runner
{
    public function run(Rows $rows): void
    {
        foreach ($rows as $row) {
            $row->@here@
        }
    }
}
"#,
        &[Query::completion("the element supplied", "here")],
        expect![[r#"
            the element supplied
            - value [method]  : int  writes `();`
            - accepting the first leaves `$row->value`
        "#]],
    );
}

#[test]
fn a_container_nothing_filled_in_falls_back_to_the_bound() {
    // The bound is what the declaration promises about every argument it could take, so its members
    // are a subset of the real ones — a miss rather than a wrong answer.
    check(
        r#"
//- /src/Bound.php
<?php
namespace App;

interface Element
{
    public function shared(): void {}
}

class Special implements Element
{
    public function shared(): void {}

    public function only(): void {}
}

/** @template T of Element */
class Holder
{
    /** @return T */
    public function first() {}
}

class Runner
{
    public function run(Holder $holder): void
    {
        $holder->first()->@here@
    }
}
"#,
        &[Query::completion("nothing supplied", "here")],
        expect![[r#"
            nothing supplied
            - shared [method]  : void  writes `();`
            - accepting the first leaves `$holder->first()->shared`
        "#]],
    );
}

/// `@return ?T` is a swap for `T` with a null the declaration added on top, so the substitution
/// happens under the null and the null stays.
#[test]
fn a_nullable_template_is_substituted_under_the_null() {
    check(
        r#"
//- /src/Nullable.php
<?php
namespace App;

class Item
{
    public function label(): string {}
}

/**
 * @template T
 */
abstract class Holder
{
    /** @return ?T */
    public function first() {}
}

/**
 * @extends Holder<Item>
 */
class Items extends Holder {}

class Runner
{
    public function run(Items $items): void
    {
        $one = $items->first();
        $on@hover@e;
        $one->@member@
    }
}
"#,
        &[
            Query::hover("what the swap left", "hover"),
            Query::completion("its members", "member"),
        ],
        expect![[r#"
            what the swap left
            - code: ?Item $one
            - note: assigned
            its members
            - label [method]  : string  writes `();`
            - accepting the first leaves `$one->label`
        "#]],
    );
}

#[test]
fn a_template_is_substituted_through_two_extends_tags() {
    // `@extends ScheduleFactory<ChildSchedule>` where `ScheduleFactory` is `@extends Factory<TModel>`
    // pins `Factory`'s own template two hops up. The substitution already worked; what hid it was
    // the `: object` hint beside `@return TModel` outranking the docblock — see
    // `TypeTable::refined`. A factory is written that way because PHP has no other way.
    check(
        r#"
//- /app.php
<?php
abstract class Model {}

/** @template TModel of Model */
class Factory
{
    /** @return TModel */
    public function create(): object {}
}

/**
 * @template TModel of Schedule
 * @extends Factory<TModel>
 */
class ScheduleFactory extends Factory {}

class Schedule extends Model {}

class ChildSchedule extends Schedule
{
    public function childMethod(): void {}
}

/** @extends ScheduleFactory<ChildSchedule> */
class ChildScheduleFactory extends ScheduleFactory {}

function run(): void
{
    $s = (new ChildScheduleFactory())->create();
    $@here@s;
    $s->$0
}
"#,
        &[
            Query::hover("through two extends", "here"),
            Query::completion("what create gives back", "0"),
        ],
        expect![[r#"
            through two extends
            - code: ChildSchedule $s
            - note: assigned
            what create gives back
            - childMethod [method]  : void  writes `();`
            - accepting the first leaves `$s->childMethod`
        "#]],
    );
}
