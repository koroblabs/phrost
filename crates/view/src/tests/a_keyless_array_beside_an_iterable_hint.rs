//! `@param Row[]` beside `iterable $rows` -- an iterable of rows, not an array of them.
//!
//! The brackets are how a docblock said "of rows" before generics, and the hint is what the runtime
//! takes: a generator or a collection gets through. Read as written the docblock won as an array, so
//! every collection handed in was reported. PHPStan reads `Row[]` and `array<Row>` beside `iterable`
//! as `iterable<Row>` and an array with its key written as the array it says, and so does this --
//! lowering writes the keyless one over the hint before either reader takes it.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_keyless_array_beside_an_iterable_hint_is_an_iterable_of_its_element() {
    check_view(
        r#"
//- /src/run.php
<?php
class Row {}

/**
 * @param Row[] $rows
 * @param array<Row>|null $maybe
 * @param list<Row> $listed
 */
function run(iterable $rows, ?iterable $maybe, iterable $listed): void
{
    $ro@rows@ws;
    foreach ($rows as $row) {
        $r@row@ow;
    }
    $ma@maybe@ybe;
    $li@listed@sted;
}

/** @return Row[] */
function made(): iterable
{
    yield new Row();
}

$m@made@ade = made();
"#,
        &[
            Query::variable_type("brackets beside iterable", "rows"),
            Query::variable_type("what iterating it gives", "row"),
            Query::variable_type("the nullable hint keeps its null", "maybe"),
            Query::variable_type("a list says its key, and stays one", "listed"),
            Query::variable_type("a return reads the same", "made"),
        ],
        expect![[r#"
            brackets beside iterable
            - iterable<Row>
            what iterating it gives
            - Row
            the nullable hint keeps its null
            - ?iterable<Row>
            a list says its key, and stays one
            - array<int, Row>
            a return reads the same
            - iterable<Row>
        "#]],
    );
}
