//! `@param Row[] ...$rows` -- the older way to say each argument is a `Row`.
//!
//! PHPStan reads it so: `rows(new Row())` passes and `rows([new Row()])` does not. Read as written,
//! every call passing one map to BookStack's `@param array<string, float>[] ...$scoreMaps` was
//! reported. A keyless array only: `list<Row> ...$rows` is a list per argument, there and here, and a
//! hint that is itself a container already says each argument is one.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn each_argument_of_a_variadic_is_the_element_of_its_documented_array() {
    check(
        r#"
//- /src/run.php
<?php
class Row {}

/** @param array<string, float>[] ...$maps */
function merge(...$maps): void {}

/** @param Row[] ...$rows */
function rows(...$rows): void {}

/** @param list<Row> ...$lists */
function lists(...$lists): void {}

/** @param int[] ...$arrays */
function arrays(array ...$arrays): void {}

merge(['a' => 1.0], ['b' => 2.0]);
merge([['a' => 1.0]]);
rows(new Row());
rows([new Row()]);
lists([new Row()]);
lists(new Row());
arrays([1], [2]);
"#,
        expect![[r#"
            17:7-17:21 error [incompatible-argument] $maps takes array<string, float>, and this is array{0: array{a: float}}
            19:6-19:17 error [incompatible-argument] $rows takes \Row, and this is array{0: Row}
            21:7-21:16 error [incompatible-argument] $lists takes array<int, \Row>, and this is Row
        "#]],
    );
}
