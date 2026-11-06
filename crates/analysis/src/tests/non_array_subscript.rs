//! An element read from a value that holds none.
//!
//! One finding and a list of subscriptable things that must stay quiet. The list is the check: a
//! `string`, an `ArrayAccess` implementer at any depth, `mixed`, and a union with one array operand
//! are all legal, and PHP code is full of every one of them.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_element_of_an_int_is_reported() {
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(int $count): mixed
    {
        return $count['total'];
    }
}
"#,
        expect![[r#"
            6:16-6:22 warning [non-array-subscript] Cannot read element 'total' of int
        "#]],
    );
}

#[test]
fn a_computed_key_is_reported_without_naming_a_slot() {
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(float $rate, string $key): mixed
    {
        return $rate[$key];
    }
}
"#,
        expect![[r#"
            6:16-6:21 warning [non-array-subscript] Cannot read an element of float
        "#]],
    );
}

#[test]
fn an_inferred_scalar_is_reported_too() {
    // Nothing declares `$count`: it is the return of a call the index resolved, which is the case
    // the check exists for -- a declared `int` is visible to a reader and an inferred one is not.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function total(): int { return 0; }
}

class Service
{
    public function run(Repo $repo): mixed
    {
        $count = $repo->total();

        return $count['x'];
    }
}
"#,
        expect![[r#"
            13:16-13:22 warning [non-array-subscript] Cannot read element 'x' of int
        "#]],
    );
}

#[test]
fn a_string_holds_characters_and_is_not_reported() {
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(string $name): string
    {
        return $name[0];
    }
}
"#,
        expect![""],
    );
}

#[test]
fn an_array_access_implementer_is_subscriptable_at_any_depth() {
    check(
        r#"
//- /stubs/ArrayAccess.php  stub
<?php
interface ArrayAccess
{
    public function offsetExists(mixed $offset): bool;
    public function offsetGet(mixed $offset): mixed;
    public function offsetSet(mixed $offset, mixed $value): void;
    public function offsetUnset(mixed $offset): void;
}

//- /src/Bag.php
<?php
namespace App;

class Bag implements \ArrayAccess
{
    public function offsetExists(mixed $offset): bool { return true; }
    public function offsetGet(mixed $offset): mixed { return null; }
    public function offsetSet(mixed $offset, mixed $value): void {}
    public function offsetUnset(mixed $offset): void {}
}

class Sack extends Bag {}

class Service
{
    public function run(Sack $sack): mixed
    {
        return $sack['app'];
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_type_that_leaves_the_question_open_is_not_reported() {
    // `mixed`, `iterable` and a union with an array operand are each a value an element may well be
    // read from, and an untyped parameter says nothing at all.
    check(
        r#"
//- /app.php
<?php
class Service
{
    /**
     * @param int|array $either
     */
    public function run(mixed $any, iterable $each, $untyped, $either): array
    {
        return [$any['a'], $each['b'], $untyped['c'], $either['d']];
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_nullable_array_is_the_null_check_s_and_only_its() {
    // Two reports for one mistake otherwise, and the other one is the more useful: the value is a
    // null nobody handled, not a type that cannot hold elements.
    //
    // This used to expect *nothing*, and the expectation was the whole bug. The rule above steps
    // aside here on the ground that the null belongs to the null check -- and the null check walked
    // member accesses, so it never reached a subscript. One finding is the right answer and it is
    // `possibly-null-subscript`; none was the answer for as long as that check did not exist.
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(?array $rows): mixed
    {
        return $rows['a'];
    }
}
"#,
        expect![[r#"
            6:16-6:21 warning [possibly-null-subscript] Cannot read element 'a' of ?array
        "#]],
    );
}

#[test]
fn writing_through_an_element_creates_the_array() {
    // `$fresh['k'] = 1` on a variable holding nothing is how PHP builds an array, and the same is
    // true where the variable held an int: the write replaces it rather than reading it.
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(int $count): array
    {
        $count['total'] = 1;
        $fresh['other'] = 2;

        return [$count, $fresh];
    }
}
"#,
        expect![""],
    );
}
