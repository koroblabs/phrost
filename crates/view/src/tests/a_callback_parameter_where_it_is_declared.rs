//! The cursor on a closure's untyped parameter, where the closure is handed to a function.
//!
//! A use of `$row` inside the body already read what the function calls the closure with; the name
//! in the signature answered "no declared type", which is §8's shape of a feature that works at a
//! use and not at the declaration of the same thing.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_declaration_answers_what_its_uses_do() {
    check_view(
        r#"
//- /stubs/standard.php  stub
<?php
/**
 * @template K as array-key
 * @template V
 * @param array<K, V> $array
 * @param null|(callable(V, K): bool)|(callable(V): bool)|(callable(K): bool) $callback
 * @return array<K, V>
 */
function array_filter(array $array, ?callable $callback = null, int $mode = 0): array {}

//- /src/app.php
<?php
namespace App;

class Row {}

/** @param list<Row> $rows */
function run(array $rows, $un@control@typed): void
{
    array_filter($rows, fn ($r@filtered@ow) => $row !== null);
    array_filter($rows, function ($p@plain@lain) { return true; });
    $untyped;
}
"#,
        &[
            Query::variable_type("an arrow function handed to array_filter", "filtered"),
            Query::variable_type("a closure handed to array_filter", "plain"),
            Query::variable_type(
                "a parameter nothing calls with anything stays untyped",
                "control",
            ),
        ],
        expect![[r#"
            an arrow function handed to array_filter
            - Row
            a closure handed to array_filter
            - Row
            a parameter nothing calls with anything stays untyped
            - (no type)
        "#]],
    );
}
