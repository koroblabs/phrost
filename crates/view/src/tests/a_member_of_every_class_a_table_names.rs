//! `$class::X` where `$class` is read out of a table of several classes.
//!
//! `Application::APPLICATION_TYPE[$type]::PARAGRAPH` in the reference project: the table maps keys
//! to `Foo::class`, and the constant is declared on every class in it and on none of their parents.
//! `new $class` already read such a value as each of the classes; `::` read it as nothing at all.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_access_reaches_the_classes_the_table_names() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

abstract class Base {}
class Order extends Base { public const PARAGRAPH = 'order'; }
class Line extends Base { public const PARAGRAPH = 'line'; }

class Types
{
    public const MAP = ['order' => Order::class, 'line' => Line::class];

    public function paragraph(string $key): string
    {
        $class = self::MAP[$key];
        return $class::PARA@paragraph@GRAPH;
    }
}
"#,
        &[Query::goto(
            "the first class in the table that declares it",
            "paragraph",
        )],
        expect![[r#"
            the first class in the table that declares it
            - const App\Order::PARAGRAPH @ src/app.php 5:41-5:50
        "#]],
    );
}

/// `new $class` on such a value, with a call after it. The instance was each of the classes, and
/// handed back early -- so the steps written after the `new` were never taken and the chain read as
/// the instance itself.
#[test]
fn a_call_on_the_instance_is_taken_after_the_new() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

abstract class Base
{
    abstract public function label(): Label;
}
class Label {}
class Order extends Base { public function label(): Label {} }
class Line extends Base { public function label(): Label {} }

class Types
{
    public const MAP = ['order' => Order::class, 'line' => Line::class];

    public function make(string $key): void
    {
        $class = self::MAP[$key];
        $i@instance@nstance = new $class();
        $l@label@abel = (new $class())->label();
    }
}
"#,
        &[
            Query::variable_type("the instance", "instance"),
            Query::variable_type("what a call on it returns", "label"),
        ],
        expect![[r#"
            the instance
            - Order|Line
            what a call on it returns
            - Label
        "#]],
    );
}
