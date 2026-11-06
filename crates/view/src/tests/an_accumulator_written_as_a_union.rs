//! A callback's accumulator written as a union of two templates, as a method writes it.
//!
//! `array_reduce` writes `callable(I, V)` and a bare `I` was refused as a class nothing declares.
//! Laravel's `reduce` writes `callable(TReduceInitial|TReduceReturnType, TValue, TKey)`, and the
//! union passed the guard -- so `$carry` read as two classes nobody declares and every member
//! access on it was an unknown class.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn an_accumulator_union_of_templates_is_no_type() {
    check_view(
        r#"
//- /src/Coll.php
<?php
namespace App;

/** @template TValue */
class Coll
{
    /**
     * @template TReduceInitial
     * @template TReduceReturnType
     * @param callable(TReduceInitial|TReduceReturnType, TValue): TReduceReturnType $callback
     * @param TReduceInitial $initial
     * @return TReduceInitial|TReduceReturnType
     */
    public function reduce(callable $callback, $initial = null) {}
}

class Runner
{
    public function go(Coll $items): void
    {
        $items->reduce(function ($carry, $item) {
            $c@carry@arry = $carry;
            return $carry;
        }, []);
    }
}
"#,
        &[Query::variable_type("the accumulator", "carry")],
        expect![[r#"
            the accumulator
            - (no type)
        "#]],
    );
}
