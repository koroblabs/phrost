//! `array_column($rows, 'name')`: one column of a container of rows.
//!
//! The stub can say only that the column is some value of some row -- `@return array<array-key,
//! V>` beside `array<K, V>` rows -- and the call says which: the field a shape names, the property
//! an object holds, each branch of a ternary between two literal keys. Before, the return was
//! bound by nothing and read as the declared `array`; 32 calls on the reference project.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_column_is_the_field_the_key_names() {
    check_view(
        r#"
//- /stubs/standard.php  stub
<?php
/**
 * @template K as array-key
 * @template V
 *
 * @param array<array-key, array<K, V>|object>|list<array<K,V>|object> $array
 * @param K|null $column_key
 * @param K|null $index_key
 *
 * @return array<array-key, V>
 */
function array_column(array $array, string|int|null $column_key, string|int|null $index_key = null): array {}

//- /src/Rows.php
<?php
namespace App;

class Item
{
    public string $label = '';
}

class Rows
{
    /**
     * @param list<array{id: int, name: string, size: ?int}> $rows
     * @param list<Item> $items
     */
    public function run(array $rows, array $items, bool $long, string $any): void
    {
        foreach (array_column($rows, 'name') as $n@name@ame) {
        }
        foreach (array_column($items, 'label') as $l@label@abel) {
        }
        foreach (array_column($rows, $long ? 'name' : 'id') as $e@either@ither) {
        }
        foreach (array_column($rows, $any) as $a@any@ny) {
        }
    }
}
"#,
        &[
            Query::variable_type("a shape's field", "name"),
            Query::variable_type("an object's property", "label"),
            Query::variable_type("either of two literal keys", "either"),
            Query::variable_type("a key nothing names", "any"),
        ],
        expect![[r#"
            a shape's field
            - string
            an object's property
            - string
            either of two literal keys
            - string|int
            a key nothing names
            - int|string|null
        "#]],
    );
}
