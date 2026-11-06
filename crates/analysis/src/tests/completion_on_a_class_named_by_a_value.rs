//! `$class::` -- a static access on a value that names a class.
//!
//! Read as `->` reads a chain, `$class = Foo::class` is a string, which has no members, so the list
//! was empty while goto -- which reads the call as a class target -- answered. A value before `::` is
//! read for the class it names: a `Foo::class` it was assigned, or a declared `class-string<Foo>`.
//! And `class` is not among what it offers, because `$class::class` on a string is a `TypeError`.

use expect_test::expect;

use crate::tests::{Query, check};

const FIXTURE: &str = r#"
//- /src/A.php
<?php
namespace App;

class Foo
{
    public const KIND = 'foo';

    public static function make(): static
    {
        return new static();
    }
}

class A
{
    /** @var class-string<Foo> */
    protected string $typed = Foo::class;

    public function assigned(): void
    {
        $class = Foo::class;
        $class::@assigned@
    }

    public function declared(): void
    {
        $this->typed::@declared@
    }
}
"#;

#[test]
fn a_value_assigned_a_class_name_offers_that_classs_statics() {
    check(
        &FIXTURE.replace("$this->typed::@declared@", "$this->typed::make();"),
        &[Query::completion("after $class::", "assigned")],
        expect![[r#"
            after $class::
            - make [method]  : static  writes `();`
            - KIND [const]  non-falsy-string
            - accepting the first leaves `$class::make`
        "#]],
    );
}

#[test]
fn a_property_declared_a_class_string_offers_that_classs_statics() {
    check(
        &FIXTURE.replace("$class::@assigned@", "$class::make();"),
        &[Query::completion("after $this->typed::", "declared")],
        expect![[r#"
            after $this->typed::
            - make [method]  : static  writes `();`
            - KIND [const]  non-falsy-string
            - accepting the first leaves `$this->typed::make`
        "#]],
    );
}
