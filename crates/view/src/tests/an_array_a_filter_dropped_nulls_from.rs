//! `array_filter($rows)` with no callback holds no nulls.
//!
//! The declaration cannot say it -- `@return array<K, V>` is a true statement about the
//! two-argument form and the only one the type language can make -- so
//! `array_values(array_unique(array_filter(array_map($cb, $types))))`, which is how PHP spells
//! *map and drop the misses*, answered `?string[]` where the author had declared `string[]`.
//!
//! The key is what decides whether the reading is allowed. `array_filter` preserves keys, and the
//! only container this layer can build is `Held::ListOf`, which answers `int` when asked what
//! iterating it yields -- so an array whose keys are strings is left alone rather than rebuilt into
//! one claiming integers.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_filtered_list_no_longer_holds_the_null_its_elements_declared() {
    check_view(
        r#"
//- /src/stubs.php
<?php

/**
 * The shape the real stub has, and the whole reason this rule exists: the declaration is a true
 * statement about the two-argument form and says nothing about what one argument leaves behind.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param null|(callable(V): bool) $callback
 *
 * @return array<K, V>
 */
function array_filter(array $array, ?callable $callback = NULL): array {}

//- /src/Rows.php
<?php
namespace App;

class Row {}

class Rows
{
    /**
     * @param list<?Row> $rows
     * @param array<string, ?Row> $keyed
     */
    public function go(array $rows, array $keyed, callable $cb): void
    {
        foreach (array_filter($rows) as $k@kept@ept) {
        }
        foreach ($rows as $u@unfiltered@nfiltered) {
        }
        foreach (array_filter($rows, $cb) as $w@callback@ithCallback) {
        }
        foreach (array_filter($keyed) as $s@keyed@tringKeyed) {
        }
    }
}
"#,
        &[
            Query::variable_type("filtered, so no null", "kept"),
            Query::variable_type("the same list unfiltered", "unfiltered"),
            Query::variable_type("a callback decides, so nothing is known", "callback"),
            Query::variable_type("string keys, so the rebuild is refused", "keyed"),
        ],
        expect![[r#"
            filtered, so no null
            - Row
            the same list unfiltered
            - ?Row
            a callback decides, so nothing is known
            - ?Row
            string keys, so the rebuild is refused
            - ?Row
        "#]],
    );
}
