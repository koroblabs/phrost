//! `array_first([$person])`: an array written out where a template reads what an array holds.
//!
//! A literal resolves as a shape -- `[$person]` is a record with a `0` in it -- and a shape has no
//! element of its own, for the keyed read's sake. So a template bound by `array<K, V>` bound nothing
//! from one, and `plain_first([$person])` answered `mixed` where `plain_first($list)` answered
//! `Person`. And a literal holding something is plainly not empty, which the conditional PHP 8.5's
//! `array_first` and `array_key_first` are written with could not see: the null branch stayed in,
//! and `strlen(array_key_first(['one' => 1]))` was reported for a null that cannot arrive.
//!
//! `array_key_first` had a second gap of its own: a branch naming a template bound by the *keys*
//! was refused at lowering, so the whole conditional went and nothing -- no literal, no guard --
//! could take the null away.

use expect_test::expect;

use crate::tests::{Query, check};

const FIXTURE: &str = r#"
//- /src/A.php
<?php
namespace App;

class Person { public string $first = ''; }

/**
 * @template V
 * @param array<array-key, V> $array
 * @return V
 */
function plain_first(array $array): mixed { return null; }

/**
 * @template V
 * @param array<array-key, V> $array
 * @return ($array is non-empty-array|non-empty-list ? V : null)
 */
function value_first(array $array): mixed { return null; }

/**
 * @template K as array-key
 * @param array<K, mixed> $array
 * @return ($array is non-empty-array|non-empty-list ? K : null)
 */
function key_first(array $array): string|int|null { return null; }

/** @param list<Person> $maybe */
function probe(Person $person, array $maybe): void
{
    $a@plain@ = plain_first([$person]);
    $b@first@ = value_first([$person]);
    $c@key@ = key_first(['one' => 1]);
    $d@spread@ = value_first([...$maybe]);
    $e@positional@ = key_first([$person]);
}

/** @param array<string, Person> $rows */
function guarded(array $rows): void
{
    if (!$rows) {
        return;
    }
    $g@guarded@ = key_first($rows);
}

function assigned(Person $person): void
{
    $rows = [$person];
    $h@assigned@ = value_first($rows);
}

/** @param list<Person> $rows */
function rewrapped(array $rows, Person $person, bool $flag): void
{
    if (!$rows) {
        return;
    }
    if ($flag) {
        $rows = [$person];
    }
    $i@rewrapped@ = value_first($rows);
}

/**
 * @param array{1: int, 2?: int} $pair
 * @param array{1?: int} $maybe
 */
function shaped(array $pair, array $maybe): void
{
    $l@shaped@ = value_first($pair);
    $m@optional@ = value_first($maybe);
}

function popped(Person $person): void
{
    $stack = [$person];
    array_pop($stack);
    $k@popped@ = value_first($stack);
}

/** @param list<Person> $rows */
function unguarded(array $rows, Person $person, bool $flag): void
{
    if ($flag) {
        $rows = [$person];
    }
    $j@unguarded@ = value_first($rows);
}
"#;

#[test]
fn a_literal_binds_the_template_and_is_not_empty() {
    check(
        FIXTURE,
        &[
            Query::hover("a template bound by the literal's values", "plain"),
            Query::hover("a non-empty literal takes the then-branch", "first"),
            Query::hover("its keys too, by PHP's rule for which key is an int", "key"),
            Query::hover("a spread may be empty, and the null stays", "spread"),
            Query::hover("a positional literal is keyed by int", "positional"),
            Query::hover(
                "a key behind a guard, which the conditional is kept for",
                "guarded",
            ),
            Query::hover("a literal written into the variable", "assigned"),
            Query::hover(
                "a guard on one path and a literal on the other",
                "rewrapped",
            ),
            Query::hover(
                "and only the literal, so the skipped path may be empty",
                "unguarded",
            ),
            Query::hover(
                "a literal a call has had since may have been emptied",
                "popped",
            ),
            Query::hover("a declared shape with a key it requires", "shaped"),
            Query::hover("and one whose every key is optional", "optional"),
        ],
        expect![[r#"
            a template bound by the literal's values
            - code: Person $a
            - note: assigned
            a non-empty literal takes the then-branch
            - code: Person $b
            - note: assigned
            its keys too, by PHP's rule for which key is an int
            - code: non-falsy-string $c
            - note: assigned
            a spread may be empty, and the null stays
            - code: ?Person $d
            - note: assigned
            a positional literal is keyed by int
            - code: int $e
            - note: assigned
            a key behind a guard, which the conditional is kept for
            - code: string $g
            - note: assigned
            a literal written into the variable
            - code: Person $h
            - note: assigned
            a guard on one path and a literal on the other
            - code: Person $i
            - note: assigned
            and only the literal, so the skipped path may be empty
            - code: ?Person $j
            - note: assigned
            a literal a call has had since may have been emptied
            - code: ?Person $k
            - note: assigned
            a declared shape with a key it requires
            - code: int $l
            - note: assigned
            and one whose every key is optional
            - code: ?int $m
            - note: assigned
        "#]],
    );
}
