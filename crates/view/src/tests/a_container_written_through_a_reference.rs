//! `foreach ($ints as &$item) { $item = 'x'; }` leaves `$ints` holding what it held or what was
//! written.
//!
//! A union, because the list may be empty and a pass may write nothing -- PHPStan reads the same
//! loop as `list<'x'|int>`. A write the element already admits changes nothing, and the container is
//! then exactly what it was declared; a write nothing can read leaves an array that says nothing of
//! its elements.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_loop_by_reference_widens_the_elements_by_what_it_wrote() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

/** @param list<int> $ints */
function strings(array $ints): void
{
    foreach ($ints as &$item) {
        $item = 'x';
    }
    unset($item);
    $s@strings@trings = $ints;
}

/** @param list<int> $ints */
function same(array $ints): void
{
    foreach ($ints as &$item) {
        $item = $item * 2;
    }
    unset($item);
    $s@same@ame = $ints;
}

/** @param array<string, int> $map */
function keyed(array $map, bool $clear): void
{
    foreach ($map as &$value) {
        if ($clear) {
            $value = null;
        }
    }
    unset($value);
    $k@keyed@eyed = $map;
}

function literal(): void
{
    $ints = [1, 2];
    foreach ($ints as &$item) {
        $item = (string) $item;
    }
    unset($item);
    $l@literal@iteral = $ints;
}

/** @param ?list<int> $ints */
function nullable(?array $ints): void
{
    foreach ($ints as &$item) {
        $item = 'x';
    }
    unset($item);
    $n@nullable@ullable = $ints;
}

/** @param list<int> $ints */
function unread(array $ints): void
{
    foreach ($ints as &$item) {
        $item = unknown_function($item);
    }
    unset($item);
    $u@unread@nread = $ints;
}
"#,
        &[
            Query::variable_type("a string written over ints", "strings"),
            Query::variable_type("an int written over ints", "same"),
            Query::variable_type("a null written on some passes", "keyed"),
            Query::variable_type("a literal's slots", "literal"),
            Query::variable_type("a container that may be null", "nullable"),
            Query::variable_type("a write nothing resolves", "unread"),
        ],
        expect![[r#"
            a string written over ints
            - array<int, int|non-falsy-string>
            an int written over ints
            - array<int, int>
            a null written on some passes
            - array<string, ?int>
            a literal's slots
            - array{0: int|non-empty-string, 1: int|non-empty-string}
            a container that may be null
            - ?array<int, int|non-falsy-string>
            a write nothing resolves
            - array
        "#]],
    );
}
