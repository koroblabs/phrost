//! What is legal inside a subscript.
//!
//! A shape is the only PHP type that names its own keys, so it is the only place a person typing
//! `$row['` can be told anything at all. Everything else offers nothing and the request carries on
//! to whatever a `[` would otherwise have asked for.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_shapes_keys_are_offered_inside_its_subscript() {
    check(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    /** @param array{app: string, count: int, note?: string} $row */
    public function run(array $row): void
    {
        $row[@here@];
    }
}
"#,
        &[Query::completion("no quote typed yet", "here")],
        expect![[r#"
            no quote typed yet
            - app [property]  string  insert `'app'`
            - count [property]  int  insert `'count'`
            - note [property]  string (optional)  insert `'note'`
            - accepting the first leaves `$row['app'];`
        "#]],
    );
}

#[test]
fn a_quote_already_typed_is_not_written_twice() {
    check(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    /** @param array{app: string, count: int} $row */
    public function run(array $row): void
    {
        $row['co@here@'];
    }
}
"#,
        &[Query::completion("filtering inside the quotes", "here")],
        expect![[r#"
            filtering inside the quotes
            - count [property]  int
            - accepting the first leaves `$row['count'];`
        "#]],
    );
}

#[test]
fn a_key_of_a_nested_shape_comes_from_the_shape_the_subscript_reached() {
    check(
        r#"
//- /src/Tree.php
<?php
namespace App;

class Tree
{
    /** @param array{outer: array{inner: int}} $tree */
    public function run(array $tree): void
    {
        $tree['outer'][@here@];
    }
}
"#,
        &[Query::completion("a shape inside a shape", "here")],
        expect![[r#"
            a shape inside a shape
            - inner [property]  int  insert `'inner'`
            - accepting the first leaves `$tree['outer']['inner'];`
        "#]],
    );
}

#[test]
fn a_value_that_is_not_a_shape_asks_for_a_name_instead() {
    // `[` opens an array literal and an attribute as well as a subscript, and a plain `array` names
    // no keys. An empty answer here would take away what the position would otherwise have offered.
    check(
        r#"
//- /src/Plain.php
<?php
namespace App;

class Plain
{
    public function run(array $rows): void
    {
        $rows[@here@];
    }
}
"#,
        &[Query::completion("a plain array", "here")],
        expect![[r#"
            a plain array
            - Plain [class]  App\Plain
            - accepting the first leaves `$rows[Plain];`
        "#]],
    );
}
