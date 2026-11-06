//! `$x::class === Foo::class` and `get_class($x) === Foo::class`: the value's class compared with
//! one written out.
//!
//! The side that holds is an `instanceof`. The other rules out `Foo` itself and not what extends it,
//! so it narrows a union only where nothing can: a final class or an enum. Over an open class a
//! subclass is still possible, and dropping the operand would offer members the value may not have.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /src/Kinds.php
<?php
namespace App;

final class Shut {}
final class Other {}
class Open {}

function equal(Shut|Other $x): void
{
    if ($x::class === Shut::class) {
        $@equal@x;
    }
    if (get_class($x) == Other::class) {
        $@called@x;
    }
}

function notEqual(Shut|Other $x): void
{
    if ($x::class !== Shut::class) {
        $@final@x;
    }
}

function earlyReturn(Shut|Other $x): void
{
    if (Shut::class === $x::class) {
        return;
    }
    $@after@x;
}

function open(Open|Other $x): void
{
    if ($x::class !== Open::class) {
        $@open@x;
    }
}
"#;

#[test]
fn the_side_that_holds_names_the_class_and_the_other_rules_out_only_a_final_one() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("::class identical", "equal"),
            Query::variable_type("get_class, loosely", "called"),
            Query::variable_type("not a final class", "final"),
            Query::variable_type("after a return, written the other way round", "after"),
            Query::variable_type("not an open class", "open"),
        ],
        expect![[r#"
            ::class identical
            - Shut
            get_class, loosely
            - Other
            not a final class
            - Other
            after a return, written the other way round
            - Other
            not an open class
            - Open|Other
        "#]],
    );
}
