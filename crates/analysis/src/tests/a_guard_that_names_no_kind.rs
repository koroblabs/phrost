//! `is_numeric($x)` and the two tests beside it, which rule a value out without naming a kind.
//!
//! All three are false for null, which is what they were read as saying. What separates them is
//! `false`: `is_numeric(FALSE)` and `is_countable(FALSE)` are both false, so the sentinel is gone --
//! and `is_scalar(FALSE)` is **true**, because a bool is a scalar, so it is not. Read as one answer,
//! `array_search($id, $keys)` under `if (is_numeric($index))` kept a `false` the guard had ruled out.
//!
//! None of them is a truthiness test either. `is_numeric('0')` is true and `'0'` is falsy, so a
//! guard that ruled every falsy value out would rule out a value these admit -- which is the third
//! state `Refused` exists to carry and what a bool could not say.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn is_numeric_rules_the_false_out_and_is_scalar_does_not() {
    check(
        r#"
//- /src/stub.php
<?php

function array_search(mixed $needle, array $haystack, bool $strict = false): string|int|false {}
function is_numeric(mixed $value): bool {}
function is_scalar(mixed $value): bool {}

//- /src/app.php
<?php

class Menu
{
    public function run(array $items, string $id): void
    {
        $numeric = array_search($id, $items);
        if (is_numeric($numeric)) {
            $nu@numeric@meric;
        }
        $scalar = array_search($id, $items);
        if (is_scalar($scalar)) {
            $sc@scalar@alar;
        }
        $unguarded = array_search($id, $items);
        $un@unguarded@guarded;
    }
}
"#,
        &[
            Query::hover("under is_numeric", "numeric"),
            Query::hover("under is_scalar", "scalar"),
            Query::hover("with no guard at all", "unguarded"),
        ],
        expect![[r#"
            under is_numeric
            - code: non-empty-string|int $numeric
            - note: narrowed by a guard
            under is_scalar
            - code: string|int|false $scalar
            - note: narrowed by a guard
            with no guard at all
            - code: string|int|false $unguarded
            - note: assigned
        "#]],
    );
}

/// What each test leaves of a union, beside the sentinels: the kinds no value passing it can be.
///
/// Laravel's `BusFake::assertDispatched($command, $callback = null)` takes `callable|int|null` and
/// hands the int on under `is_numeric($callback)` -- which no callable passes, since a function's
/// name cannot start with a digit. `is_countable` leaves no scalar, and `is_scalar` no object or
/// array. Each one only filters what the declaration listed: a value nothing described stays so.
#[test]
fn each_test_rules_out_the_kinds_nothing_passing_it_can_be() {
    check(
        r#"
//- /src/stub.php
<?php

function is_numeric(mixed $value): bool {}
function is_countable(mixed $value): bool {}
function is_scalar(mixed $value): bool {}

//- /src/app.php
<?php

class Bus
{
    /**
     * @param callable|int|null $callback
     */
    public function numeric(\Closure $command, $callback = null): void
    {
        if (is_numeric($callback)) {
            $call@numeric@back;
        }
    }

    public function countable(string|array $rows): void
    {
        if (is_countable($rows)) {
            $ro@countable@ws;
        }
    }

    public function scalar(string|\Closure|array|null $title): void
    {
        if (is_scalar($title)) {
            $ti@scalar@tle;
        }
    }

    public function untyped($value): void
    {
        if (is_numeric($value)) {
            $va@untyped@lue;
        }
    }
}
"#,
        &[
            Query::hover("callable|int|null under is_numeric", "numeric"),
            Query::hover("string|array under is_countable", "countable"),
            Query::hover("string|Closure|array|null under is_scalar", "scalar"),
            Query::hover("a value nothing described", "untyped"),
        ],
        expect![[r#"
            callable|int|null under is_numeric
            - code: int $callback
            - note: narrowed by a guard
            string|array under is_countable
            - code: array $rows
            - note: narrowed by a guard
            string|Closure|array|null under is_scalar
            - code: string $title
            - note: narrowed by a guard
            a value nothing described
            - code: $value
            - note: narrowed by a guard
            - note: no declared type
        "#]],
    );
}

/// The path that skipped a write the test guarded is where the test held, and says all of it.
///
/// WordPress's `current_time()` normalises an `int|bool` that way: `if (!is_numeric($timestamp))`
/// replaces it with an int, so around the branch it was numeric -- no `false`, and no `true`
/// either. When the kinds were claims nested on the sentinel test, the path around the write was
/// described by the outermost one alone and the `false` came back.
#[test]
fn the_path_around_the_write_is_where_the_test_held() {
    check(
        r#"
//- /src/stub.php
<?php

function is_numeric(mixed $value): bool {}
function time(): int {}

//- /src/app.php
<?php

function current(int|bool $timestamp): void
{
    if (!is_numeric($timestamp)) {
        $timestamp = time();
    }
    $time@after@stamp;
}
"#,
        &[Query::hover("past the normalising branch", "after")],
        expect![[r#"
            past the normalising branch
            - code: int $timestamp
            - note: assigned
        "#]],
    );
}

/// `is_countable([])` is true, so an empty shape is still there afterwards -- the one falsy value a
/// truthiness guard takes and this test does not.
#[test]
fn a_countable_test_keeps_the_empty_shape() {
    check(
        r#"
//- /src/stub.php
<?php

function is_countable(mixed $value): bool {}

//- /src/app.php
<?php

class Address
{
    /**
     * @param array{city: string}|array{}|string $address
     */
    public function read($address): void
    {
        if (is_countable($address)) {
            $add@countable@ress;
        }
    }
}
"#,
        &[Query::hover("under is_countable", "countable")],
        expect![[r#"
            under is_countable
            - code: array{city: string}|array{} $address
            - note: narrowed by a guard
        "#]],
    );
}
