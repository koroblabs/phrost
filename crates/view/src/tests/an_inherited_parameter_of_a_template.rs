//! An untyped parameter of an override, under the interface's `@param TSet|null $value`.
//!
//! The climb found the declaration and kept who wrote it, and then handed the type back with no
//! substitution at all -- so `$value` in the body read as a class called `TSet`, whether or not an
//! `@implements` above said what it is. Laravel's `CastsAttributes::set` is the shape.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn an_inherited_parameter_takes_what_the_implements_supplied() {
    check_view(
        r#"
//- /src/Casts.php
<?php
namespace App;

class Address {}

/** @template TSet */
interface Casts
{
    /** @param TSet|null $value */
    public function set($model, $value);
}

class Bare implements Casts
{
    public function set($model, $value)
    {
        $b@bare@are = $value;
    }
}

/** @implements Casts<Address> */
class Bound implements Casts
{
    public function set($model, $value)
    {
        $b@bound@ound = $value;
    }
}
"#,
        &[
            Query::variable_type("nothing supplied it", "bare"),
            Query::variable_type("`@implements Casts<Address>`", "bound"),
        ],
        expect![[r#"
            nothing supplied it
            - (no type)
            `@implements Casts<Address>`
            - ?Address
        "#]],
    );
}
