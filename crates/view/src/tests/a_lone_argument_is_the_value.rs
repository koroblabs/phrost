//! One type argument written to a class whose value is the second of its templates.
//!
//! `Collection<Page>` gives a `<TKey, TValue>` collection one argument, and it is the value -- a
//! collection of pages, whose keys it does not say. Read by position the page was the key and the
//! value was nothing, so a `foreach` over it typed nothing. PHPStan, asked on BookStack, answers the
//! page for the value and `int|string` for the key, which is the reading here: the value, and no
//! claim about the key.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_lone_argument_is_what_iterating_yields() {
    check_view(
        r#"
//- /src/Coll.php
<?php
namespace App;

class Item {}

/**
 * @template TKey of array-key
 * @template TValue
 * @implements \IteratorAggregate<TKey, TValue>
 */
class Coll implements \IteratorAggregate
{
    public function getIterator(): \Iterator {}
}

class Shelf
{
    /** @return Coll<Item> */
    public function items(): Coll {}

    /** @return Coll<int, Item> */
    public function paired(): Coll {}
}

/** @param Coll<Item> $written */
function run(Shelf $shelf, Coll $written): void
{
    foreach ($shelf->items() as $k@stored_key@ey => $s@stored@tored) {}
    $x@param_itself@x = $written;
    foreach ($written as $e@written@ach) {}
    foreach ($shelf->paired() as $p@paired_key@airedKey => $p@paired@aired) {}
}
"#,
        &[
            Query::variable_type("a declared return written with one argument", "stored"),
            Query::variable_type("and its key, which the argument does not say", "stored_key"),
            Query::variable_type("the parameter itself", "param_itself"),
            Query::variable_type("a parameter written with one argument", "written"),
            Query::variable_type("a pair, as before", "paired"),
            Query::variable_type("and its key", "paired_key"),
        ],
        expect![[r#"
            a declared return written with one argument
            - Item
            and its key, which the argument does not say
            - (no type)
            the parameter itself
            - Coll<Item>
            a parameter written with one argument
            - Item
            a pair, as before
            - Item
            and its key
            - int
        "#]],
    );
}
