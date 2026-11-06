//! `BcMath\Number` and `GMP` under the arithmetic operators, which both overload.
//!
//! Each answers an object of its own class whatever number stands beside it, and a `TypeError` --
//! no value at all -- beside a null, an array or the other class. Asked of PHP 8.5.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn an_overloading_class_makes_an_object_of_itself() {
    check_view(
        r#"
//- /stubs/bcmath.php  stub
<?php
namespace BcMath;
final readonly class Number implements \Stringable {}

//- /stubs/gmp.php  stub
<?php
final class GMP {}

//- /src/app.php
<?php
namespace App;

use BcMath\Number;

function run(
    Number $price,
    int $quantity,
    \GMP $left,
    \GMP $right,
    ?Number $maybe,
    Number|int $either,
    array $items,
    \DateTime $when,
): void {
    $p@product@roduct = $price * $quantity;
    $s@swapped@wapped = 2 * $price;
    $f@float@loat = $price + 1.5;
    $q@quotient@uotient = $price / $quantity;
    $g@gmp@mp = $left + $right;
    $n@negated@egated = -$left;
    $m@maybe@aybe = $maybe * 2;
    $e@either@ither = $either + 1;
    $t@two@wo = $price + $left;
    $a@array@rray = $price + $items;
    $o@other@ther = $when * 2;
    $i@stepped@tepped = ++$left;
}
"#,
        &[
            Query::variable_type("a Number times an int", "product"),
            Query::variable_type("an int times a Number", "swapped"),
            Query::variable_type("a Number plus a float", "float"),
            Query::variable_type("a Number divided", "quotient"),
            Query::variable_type("two GMPs", "gmp"),
            Query::variable_type("a GMP negated", "negated"),
            Query::variable_type("a null is the 0 it makes", "maybe"),
            Query::variable_type("a Number or an int", "either"),
            Query::variable_type("the two classes together", "two"),
            Query::variable_type("a Number beside an array", "array"),
            Query::variable_type("a class that does not overload", "other"),
            Query::variable_type("a GMP stepped", "stepped"),
        ],
        expect![[r#"
            a Number times an int
            - Number
            an int times a Number
            - Number
            a Number plus a float
            - Number
            a Number divided
            - Number
            two GMPs
            - GMP
            a GMP negated
            - GMP
            a null is the 0 it makes
            - Number|int
            a Number or an int
            - Number|int
            the two classes together
            - (no type)
            a Number beside an array
            - (no type)
            a class that does not overload
            - (no type)
            a GMP stepped
            - GMP
        "#]],
    );
}
