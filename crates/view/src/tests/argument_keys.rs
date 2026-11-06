//! `array_keys($rows)` -- a container of what names each slot of the argument.
//!
//! The one shape the element branch cannot express, and the reason is positional: `@param array<K, V>
//! $array` names both halves and only which argument it is tells them apart, so the reader that takes
//! the *last* one answers about the values. `array_keys` is written `@return list<K>` and asks about
//! the first.
//!
//! What a key *is* does not follow from the argument either. A `list<Row>` is keyed by `int` and an
//! `array<string, Row>` by `string`, and neither writes that anywhere -- it follows from how the
//! container was spelled, which is the same question `foreach ($x as $k)` asks and the same answer.
//!
//! The bare `array` is the half worth pinning: its keys really are `int|string`, nothing in the
//! source narrows that, and answering `int` would be a type the value does not have carried into
//! every use of `$k` downstream. The contract declines, so what is left is the
//! declaration's own bare `array`.
//!
//! `Row[]` is the *same* half, and this file used to pin `int` for it under a case named "brackets
//! are PHP's numbering" -- which is the premise rather than a measurement, and the premise is wrong:
//! brackets name a container and nothing about its slots, which is exactly what the paragraph above
//! says about a bare `array`. PSR-7's `getHeaders()` is `@return string[][]` and is keyed by header
//! name; the invented `int` made `strtolower($name)` an error on three correct lines.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_keys_of_a_container_follow_from_how_it_was_written() {
    check_view(
        r#"
//- /stubs/builtins.php  stub
<?php
/**
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @return ($array is non-empty-array|non-empty-list ? non-empty-list<K> : list<K>)
 */
function array_keys(array $array): array {}

//- /src/Row.php
<?php
namespace App;

class Row
{
    public function id(): int
    {
        return 0;
    }
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    /**
     * @param list<Row> $rows
     * @param array<string, Row> $map
     * @param Row[] $short
     */
    public function run(array $rows, array $map, array $short, array $bare): void
    {
        $numbered = array_keys($rows);
        $nu@numbered@mbered;
        $named = array_keys($map);
        $na@named@med;
        $brackets = array_keys($short);
        $br@brackets@ackets;
        $unsaid = array_keys($bare);
        $un@unsaid@said;
    }
}
"#,
        &[
            Query::variable_type("a list is numbered", "numbered"),
            Query::variable_type("a map says its key type", "named"),
            Query::variable_type("brackets say nothing either", "brackets"),
            Query::variable_type("a bare array says nothing", "unsaid"),
        ],
        expect![[r#"
            a list is numbered
            - int[]
            a map says its key type
            - string[]
            brackets say nothing either
            - array<int, mixed>
            a bare array says nothing
            - array<int, mixed>
        "#]],
    );
}

#[test]
fn a_single_key_follows_the_same_rule_and_may_be_missing() {
    // `array_key_first`, with the conditional exactly as the prelude writes it -- `K` for an array
    // known to be non-empty and `null` otherwise. The null is set aside as the value not being
    // there, as `reset()`'s `false` is, so the two branches agree on `K` and the answer is `?K`.
    check_view(
        r#"
//- /stubs/builtins.php  stub
<?php
/**
 * @template K as array-key
 *
 * @param array<K, mixed> $array
 *
 * @return ($array is non-empty-array|non-empty-list ? K : null)
 */
function array_key_first(array $array): string|int|null {}

//- /src/Caller.php
<?php
namespace App;

class Row {}

class Caller
{
    /**
     * @param list<Row> $rows
     * @param array<string, Row> $map
     */
    public function run(array $rows, array $map, array $bare): void
    {
        $f@numbered@irst = array_key_first($rows);
        $n@named@amed = array_key_first($map);
        $u@unsaid@nsaid = array_key_first($bare);
    }
}
"#,
        &[
            Query::variable_type("a list is numbered", "numbered"),
            Query::variable_type("a map is named", "named"),
            Query::variable_type("a bare array says what the declaration says", "unsaid"),
        ],
        expect![[r#"
            a list is numbered
            - ?int
            a map is named
            - ?string
            a bare array says what the declaration says
            - string|int|null
        "#]],
    );
}
