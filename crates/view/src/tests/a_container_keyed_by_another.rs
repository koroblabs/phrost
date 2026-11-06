//! `array_combine($names, $lengths)` -- keyed by what one argument holds, holding what the other
//! does.
//!
//! The stubs write `@return array<K, V>` beside `@param array<K> $keys` and `@param array<V>
//! $values`, under a conditional about emptiness. The elements branch refused it, because the key
//! is not the values argument's own -- reading it as `array_values` would have handed `foreach
//! ($m as $k => $v)` an `int` key where the call says `string`. So the result carries both halves,
//! and a keys argument that says nothing about its elements leaves the key what any key is,
//! `int|string`, rather than the `int` a list would answer.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_key_is_what_the_first_argument_holds() {
    check_view(
        r#"
//- /stubs/builtins.php  stub
<?php
/**
 * @template K as array-key
 * @template V
 *
 * @param array<K> $keys
 * @param array<V> $values
 *
 * @return ($keys is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>)
 */
function array_combine(array $keys, array $values): array {}

//- /src/Caller.php
<?php
namespace App;

class Row {}

class Caller
{
    /**
     * @param list<string> $names
     * @param list<Row> $rows
     */
    public function run(array $names, array $rows, array $bare): void
    {
        $byName = array_combine($names, $rows);
        $b@combined@yName;
        foreach ($byName as $k => $v) {
            $k@key@;
            $v@value@;
        }
        $unkeyed = array_combine($bare, $rows);
        $u@unkeyed@nkeyed;
    }
}
"#,
        &[
            Query::variable_type("the combination", "combined"),
            Query::variable_type("its key", "key"),
            Query::variable_type("its value", "value"),
            Query::variable_type("keys that say nothing leave any key", "unkeyed"),
        ],
        expect![[r#"
            the combination
            - array<string, Row>
            its key
            - string
            its value
            - Row
            keys that say nothing leave any key
            - array<int|string, Row>
        "#]],
    );
}
