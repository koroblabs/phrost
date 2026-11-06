//! `Assert::isInstanceOf($x, Foo::class);` on its own line, which is how a test narrows.
//!
//! An unconditional `@phpstan-assert` is read at the call itself -- nobody looked at what the call
//! returned, so there is no branch to check a polarity against. What it was gated on was the
//! *value*: only one that says little -- possibly null, or of no known class -- was asked, on the
//! ground that nobody writes a contract about a value that already says what it is.
//!
//! `assertInstanceOf` is exactly that, and is the commonest assertion there is: it narrows a
//! declared interface to the concrete class the test just built. 326 findings on one real checkout
//! sat inside the assertion that ruled them out. The gate is the *call list* now -- whether the
//! body hands this value to any statement call at all, which is no for nearly every value in a
//! file -- and the value's own precision is kept as a separate rule: an assertion may only make
//! the answer narrower.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_assertion_narrows_a_value_that_already_knows_its_class() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base {}

class Special extends Base
{
    public function only(): string
    {
        return 'o';
    }
}

class Asserts
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert =ExpectedType $actual
     */
    public static function assertInstanceOf(string $expected, mixed $actual): void {}
}

function test(Base $value): string
{
    Asserts::assertInstanceOf(Special::class, $value);

    return $value->only();
}
"#,
        expect![""],
    );
}

/// And may only narrow. `assertSame($expected, $actual)` asserts the *expected* side's type onto
/// the value, so where that side is declared by an interface the subclass would be taken away --
/// and every method the test then calls on it reported.
#[test]
fn an_assertion_naming_an_ancestor_leaves_the_value_alone() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

interface Holder {}

class MutableBag implements Holder
{
    public function add(): string
    {
        return 'a';
    }
}

class Asserts
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert =ExpectedType $actual
     */
    public static function assertInstanceOf(string $expected, mixed $actual): void {}
}

function test(): string
{
    $value = new MutableBag();
    Asserts::assertInstanceOf(Holder::class, $value);

    return $value->add();
}
"#,
        expect![""],
    );
}

/// The control for the two empty expectations above: the same call with no tag on it reports, so
/// an empty snapshot there is the assertion being read rather than the harness saying nothing.
#[test]
fn the_same_call_without_the_tag_reports() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base {}

class Special extends Base
{
    public function only(): string
    {
        return 'o';
    }
}

class Asserts
{
    public static function assertInstanceOf(string $expected, mixed $actual): void {}
}

function test(Base $value): string
{
    Asserts::assertInstanceOf(Special::class, $value);

    return $value->only();
}
"#,
        expect![[r#"
            23:20-23:24 error [undefined-method] Undefined method App\Base::only()
        "#]],
    );
}

/// An assignment written as the asserted argument asserts the variable it wrote:
/// `assertInstanceOf(Pivot::class, $fresh = $pivot->fresh())`. The proof was about the assignment
/// expression, which no later read is, so the variable kept the `?Pivot` the call declared.
#[test]
fn an_assignment_asserted_in_place_narrows_the_variable_it_wrote() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Pivot
{
    public ?string $level = null;

    public function fresh(): ?static
    {
        return $this;
    }
}

class Asserts
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert =ExpectedType $actual
     */
    public static function assertInstanceOf(string $expected, mixed $actual): void {}
}

function test(Pivot $pivot): ?string
{
    Asserts::assertInstanceOf(Pivot::class, $fresh = $pivot->fresh());

    return $fresh->level;
}
"#,
        expect![""],
    );
}
