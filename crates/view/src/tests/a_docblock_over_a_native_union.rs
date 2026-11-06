//! `array|false $lines` beside `@param list<string>|false $lines` -- a docblock giving a native
//! union's container its element.
//!
//! A docblock refined a bare `array` and its nullable spelling, and never a union, so the most common
//! signature in the standard library -- `array|false`, with the element written beside it -- typed
//! nothing iterated. The rule is the lone one operand by operand: every container in the hint gets
//! its arguments, and every other operand is restated. A docblock that drops one the runtime admits,
//! or adds one it refuses, is a competing claim and loses, as it does for a lone type.
//!
//! Both representations decide this, the written one inside the body and the interned one at a call,
//! so both are asked here.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_docblock_gives_each_container_of_a_native_union_its_element() {
    check_view(
        r#"
//- /src/run.php
<?php
class Row {}

/**
 * @param list<Row>|false $failed
 * @param Row[]|string|null $either
 * @param array{row: Row}|array{} $shaped
 * @param list<Row> $dropped
 * @return list<Row>|false
 */
function rows(array|false $failed, array|string|null $either, array $shaped, array|false $dropped): array|false
{
    foreach ($failed as $one) {
        $o@failed@ne;
    }
    $ei@either@ther;
    $sh@shaped@aped;
    foreach ($dropped as $three) {
        $th@dropped@ree;
    }
    return [];
}

foreach (rows([], [], [], []) as $four) {
    $fo@called@ur;
}
"#,
        &[
            Query::variable_type("beside false", "failed"),
            Query::variable_type("beside a string and a null", "either"),
            Query::variable_type("a union of shapes beside a bare array", "shaped"),
            Query::variable_type("a docblock that drops the false", "dropped"),
            Query::variable_type("what a call hands back", "called"),
        ],
        expect![[r#"
            beside false
            - Row
            beside a string and a null
            - Row[]|string|null
            a union of shapes beside a bare array
            - array{row: Row}|array{}
            a docblock that drops the false
            - (no type)
            what a call hands back
            - Row
        "#]],
    );
}
