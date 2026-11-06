//! `$x .= ...`, `$x &= ...`, `$x += ...`: a write whose value the operator decides.
//!
//! Lowered as a write so that renaming and inlining see one, it was still invisible to every reader
//! asking what a name holds -- so `$ok = true; $ok &= $step;` stayed `true`, and a method declared
//! `bool` returning it read as returning one, where PHP hands back the `int` PHPStan reports.
//! `.=` is a string, the bitwise operators and `%=` an int, and the arithmetic ones are the same
//! arithmetic `$a + $b` is, over the target's own value. Carried round a loop, a compound write
//! reads what it wrote on the pass before, so it joins as the fixed point it is rather than
//! resolving through the loop again.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_operator_decides_what_the_name_holds() {
    check_view(
        r#"
//- /src/Totals.php
<?php
namespace App;

class Row
{
    public float $amount = 0.0;
}

class Totals
{
    /** @param list<Row> $rows */
    public function run(array $rows, bool $step, int $id): void
    {
        $ok = true;
        $ok &= $step;
        $o@bitwise@k;

        $label = 42;
        $label .= $id;
        $l@concat@abel;

        $half = 1;
        $half /= 2;
        $h@divided@alf;

        $total = 0;
        foreach ($rows as $row) {
            $total += $row->amount;
        }
        $t@looped@otal;
    }
}
"#,
        &[
            Query::variable_type("`&=`", "bitwise"),
            Query::variable_type("`.=`", "concat"),
            Query::variable_type("`/=`", "divided"),
            Query::variable_type("`+=` carried round a loop", "looped"),
        ],
        expect![[r#"
            `&=`
            - int
            `.=`
            - non-empty-string
            `/=`
            - int|float
            `+=` carried round a loop
            - float|int
        "#]],
    );
}
