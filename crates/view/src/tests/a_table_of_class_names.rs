//! A constant table of `Foo::class` values, and `new` on what it holds.
//!
//! `const TYPES = ['order' => Order::class]` is how PHP writes a factory's map, and the table is a
//! `array<string, class-string<Order>>` -- also where the constant is typed a bare `array`, which the
//! value refines as a docblock refines a hint. `new (self::TYPES[$k])` is then a `Order`, and over a
//! table of several classes an instance of any of them -- a union, not their parent:
//! `Types::MAP[$k]::PARAGRAPH` reads a constant each of them declares and the parent does not, so
//! no single class stands for the value.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_table_of_one_class_is_instantiated_as_it() {
    check_view(
        r#"
//- /src/Factory.php
<?php
namespace App;

abstract class Base {}
class Order extends Base {}
class Line extends Base {}

class Factory
{
    public const array ONE = ['order' => Order::class];
    const MANY = ['order' => Order::class, 'line' => Line::class];

    public function make(string $kind): void
    {
        $class = self::ONE[$kind];
        $o@one@ne = new $class();
        $several = self::MANY[$kind];
        $s@several_classes@everal;
        $m@many@any = new $several();
    }
}
"#,
        &[
            Query::variable_type("one class in the table", "one"),
            Query::variable_type("what several classes are", "several_classes"),
            Query::variable_type("new on several", "many"),
        ],
        expect![[r#"
            one class in the table
            - Order
            what several classes are
            - class-string<Order|Line>
            new on several
            - Order|Line
        "#]],
    );
}
