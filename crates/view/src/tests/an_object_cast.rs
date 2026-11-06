//! `(object) $value`: the object it already was, or a `stdClass` PHP builds around anything else.
//!
//! The cast states `object` and nothing narrower, so `(object) ['a' => 1]` was a value whose class
//! nobody could name. PHP 8.5.10 turns an array, a scalar and a null alike into a `stdClass`, and
//! hands an object back untouched -- so a `?Foo` could be either, and says only `object`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn what_the_cast_makes_of_each_kind() {
    check_view(
        r#"
//- /stubs/core.php  stub
<?php
class stdClass {}

//- /src/app.php
<?php
namespace App;

class Foo {}

function run(array $a, Foo $f, ?Foo $n, $u, int $i): void
{
    $l@literal@it = (object) ['a' => 1];
    $r@array@ows = (object) $a;
    $o@object@bj = (object) $f;
    $m@maybe@aybe = (object) $n;
    $x@unknown@x = (object) $u;
    $s@scalar@calar = (object) $i;
}
"#,
        &[
            Query::variable_type("an array literal", "literal"),
            Query::variable_type("an array", "array"),
            Query::variable_type("an object stays itself", "object"),
            Query::variable_type("an object or the null", "maybe"),
            Query::variable_type("a value nothing describes", "unknown"),
            Query::variable_type("a scalar", "scalar"),
        ],
        expect![[r#"
            an array literal
            - stdClass
            an array
            - stdClass
            an object stays itself
            - Foo
            an object or the null
            - object
            a value nothing describes
            - object
            a scalar
            - stdClass
        "#]],
    );
}
