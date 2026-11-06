//! A constant table read by the key a call passes: `@return ID_TABLE[T]` beside `@param T $type`.
//!
//! Two stored halves, each held here because both invariants compare the dump. The contract names
//! the table and the parameter; the table's own type says what each key holds -- which for values of
//! different kinds was the plain `array`, so no key could be read off it at all.

use expect_test::expect;

use super::check_index;

#[test]
fn a_table_whose_values_differ_in_kind_is_the_shape_of_its_keys() {
    check_index(
        r#"
//- /src/tables.php
<?php
namespace App;

const ID_TABLE = ['immutable' => 1, 'mutable' => 'two'];
const UNIFORM = ['thumb' => 200, 'full' => 1200];
const LISTED = [1, 'two', true];
const COMPUTED = [PHP_EOL => 1, 'b' => 'two'];
const REPEATED = ['a' => 1, 'a' => 'two'];
const NULL_LAST = ['a' => 1, 'b' => 'two', 'c' => null];
"#,
        expect![[r#"
            state ready
            file src/tables.php [User]
              constant App\ID_TABLE @ 4:7-4:15
              constant App\UNIFORM @ 5:7-5:14
              constant App\LISTED @ 6:7-6:13
              constant App\COMPUTED @ 7:7-7:15
              constant App\REPEATED @ 8:7-8:15
              constant App\NULL_LAST @ 9:7-9:16
              constant App\ID_TABLE: array{immutable: int, mutable: non-falsy-string}
              constant App\UNIFORM: array<non-falsy-string, int>
              constant App\LISTED: list{0: int, 1: non-falsy-string, 2: bool}
              constant App\COMPUTED: array
              constant App\REPEATED: array
              constant App\NULL_LAST: array
            catalog
              [const] App\COMPUTED -> src/tables.php (App\COMPUTED)
              [const] App\ID_TABLE -> src/tables.php (App\ID_TABLE)
              [const] App\LISTED -> src/tables.php (App\LISTED)
              [const] App\NULL_LAST -> src/tables.php (App\NULL_LAST)
              [const] App\REPEATED -> src/tables.php (App\REPEATED)
              [const] App\UNIFORM -> src/tables.php (App\UNIFORM)
            namespaces
              app -> App x6
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_return_naming_a_constant_at_its_template_is_an_entry_of_the_table() {
    check_index(
        r#"
//- /src/Table.php
<?php
namespace App;

const ID_TABLE = ['immutable' => 1, 'mutable' => 'two'];

class Table
{
    public const TABLE = ['one' => 1, 'two' => 'zwei'];

    /**
     * @template T of key-of<self::TABLE>
     * @param T $type
     * @return self::TABLE[T]
     */
    public static function get(string $type = 'one'): int|string {}

    /**
     * @template T of key-of<ID_TABLE>
     * @param T $type
     * @return ID_TABLE[T]
     */
    public function lookUp(string $type = 'immutable'): int|string {}
}
"#,
        expect![[r#"
            state ready
            file src/Table.php [User]
              class App\Table @ 6:7-6:12
              constant App\ID_TABLE @ 4:7-4:15
              constant App\ID_TABLE: array{immutable: int, mutable: non-falsy-string}
              App\Table is class
              App\Table::const TABLE  public type from value  @ 8:18-8:23 (full 8:5-8:56)
              App\Table::method get(string $type = 'one'): int|string  public static return names its own template returns the entry of self::TABLE at the key argument 0 is  @ 15:28-15:31 (full 15:5-15:68)
              App\Table::method lookUp(string $type = 'immutable'): int|string  public return names its own template returns the entry of App\ID_TABLE or ID_TABLE at the key argument 0 is  @ 22:21-22:27 (full 22:5-22:70)
            catalog
              [class] app\table -> src/Table.php (App\Table)
              [const] App\ID_TABLE -> src/Table.php (App\ID_TABLE)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}
