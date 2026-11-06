//! A member reached through a union whose operands are intersections.
//!
//! `$product = $c ? $this->createMock(PriceListProduct::class) : $this->createMock(Product::class)`
//! -- written with an `if` in the reference project's own tests -- holds
//! `MockObject&PriceListProduct|MockObject&Product`. Neither operand names one class, so the
//! operand that declares `method()` was never found, and every stubbed call on the mock was an
//! untyped receiver. The parts of each intersection are asked instead.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_part_both_operands_carry_answers_the_member() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

interface Mock
{
    public function method(string $name): Stub;
}
class Stub {}
class Product {}
class PriceList extends Product {}

function credit(): Mock&PriceList {}
function plain(): Mock&Product {}

function run(bool $c): void
{
    if ($c) {
        $product = credit();
    } else {
        $product = plain();
    }
    $stub = $product->method('total');
    $s@stub@tub;
}
"#,
        &[Query::variable_class(
            "what the shared part's method returns",
            "stub",
            "stub",
        )],
        expect![[r#"
            what the shared part's method returns
            - App\Stub
        "#]],
    );
}
