//! A method's `@template` with a default stands for its default where the call leaves its argument
//! out.
//!
//! `@template TDefault = never` bound by a parameter that defaults to `null`: PHP passes the `null`,
//! but the template was declared to be nothing in that case, which is the whole point of writing it.
//! Guzzle's promises, Laravel's `whenHas` and the stub set's `Ds\Map::get` are the shape, and reading
//! the parameter's default answered `?TValue` for a call that cannot return null -- and reported
//! every method called on the result.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_template_left_out_is_its_default() {
    check_view(
        r#"
//- /src/Store.php
<?php
namespace App;

class Foo {}

/** @template TValue */
class Store
{
    /**
     * @template TDefault = never
     * @param TDefault $default
     * @return TValue|TDefault
     */
    public function get(string $key, mixed $default = null): mixed {}

    /**
     * @template TDefault
     * @param TDefault $default
     * @return TValue|TDefault
     */
    public function plain(string $key, mixed $default = null): mixed {}
}

class Runner
{
    /** @param Store<Foo> $store */
    public function go(Store $store): void
    {
        $l@left@eft = $store->get('k');
        $p@passed@assed = $store->get('k', 5);
        $d@plain@lain = $store->plain('k');
    }
}
"#,
        &[
            Query::variable_type("a template left out", "left"),
            Query::variable_type("the same template passed", "passed"),
            Query::variable_type("no default: the parameter's own", "plain"),
        ],
        expect![[r#"
            a template left out
            - Foo
            the same template passed
            - Foo|int
            no default: the parameter's own
            - ?Foo
        "#]],
    );
}
