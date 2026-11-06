//! `@return ID_TABLE[T]` beside `@param T $type`: the entry of a constant's table at the key a call
//! passes, the default included.
//!
//! PHPStan's offset access on a constant. A table of one `int` and one `string` read as the plain
//! `array` the syntax could not type, so every call answered the declared `int|string` -- and
//! `takesInt(lookUp('immutable'))` was reported on correct code. The table is a constant written in
//! a global namespace or on a class; it is one rule, and both are asked here.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn each_key_reads_its_own_entry() {
    check_view(
        r#"
//- /src/Table.php
<?php
namespace App;

const ID_TABLE = ['immutable' => 1, 'mutable' => 'two'];

/**
 * @template T of key-of<ID_TABLE>
 * @param T $type
 * @return ID_TABLE[T]
 */
function lookUp(string $type = 'immutable'): int|string {}

final class Table
{
    public const TABLE = ['one' => 1, 'two' => 'zwei'];

    /**
     * @template T of key-of<self::TABLE>
     * @param T $type
     * @return self::TABLE[T]
     */
    public static function get(string $type = 'one'): int|string {}
}

/**
 * @template T of key-of<Table::TABLE>
 * @param T $type
 * @return Table::TABLE[T]
 */
function viaClass(string $type): int|string {}

class Caller
{
    public function run(string $unknown): void
    {
        $immutable = lookUp('immutable');
        $i@immutable@mmutable;
        $mutable = lookUp('mutable');
        $m@mutable@utable;
        $defaulted = lookUp();
        $d@defaulted@efaulted;
        $own = Table::get('two');
        $o@own@wn;
        $named = viaClass('one');
        $n@named@amed;
        $whole = Table::TABLE;
        $w@whole@hole;
        $any = lookUp($unknown);
        $a@any@ny;
    }
}
"#,
        &[
            Query::variable_type("the int entry", "immutable"),
            Query::variable_type("the string entry", "mutable"),
            Query::variable_type("the default's entry", "defaulted"),
            Query::variable_type("a class's own table through self", "own"),
            Query::variable_type("a class's table named outright", "named"),
            Query::variable_type("the table itself", "whole"),
            Query::variable_type("a key nothing names leaves the declaration", "any"),
        ],
        expect![[r#"
            the int entry
            - int
            the string entry
            - non-falsy-string
            the default's entry
            - int
            a class's own table through self
            - non-falsy-string
            a class's table named outright
            - int
            the table itself
            - array{one: int, two: non-falsy-string}
            a key nothing names leaves the declaration
            - int|string
        "#]],
    );
}
