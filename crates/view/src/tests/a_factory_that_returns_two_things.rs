//! `@return MockObject&T` -- a value that is two things at once, and both of them wanted.
//!
//! Every mock a PHPUnit test makes is written this way: `createMock(string $type): MockObject` with
//! `@return MockObject&RealInstanceType`, and `getMockBuilder(...)->getMock()` with the same tag on
//! the builder's own template. The test then calls the *mocked* class's methods through the value
//! and PHPUnit's on the same value, and reading either half alone reports the other: **446 findings
//! on one framework's tests**, `undefined-method` and `undefined-property` both.
//!
//! Two readers had to learn it, because a template is bound two ways and §8 already records that as
//! a form. Lowering reads the tag for a template bound by a `class-string<T>` **argument**, and it
//! matched a bare name only -- so an intersection built no contract at all; both operands are
//! written as names and only the parameter list says which is the template, so both are asked and
//! the real class answers nothing. And the substitution that reads a template bound by the
//! **receiver** walked a union operand by operand and declined an intersection, which is the same
//! walk with the other fold.
//!
//! Nothing is interned to say it. `Held::AllOf` carries the two claims, which is what a reading view
//! can build where the type table -- open for reading only -- cannot.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /src/Mocks.php
<?php
namespace App;

interface Real
{
    public function realMethod(): int;
}

class Doubled
{
    public function expects(string $matcher): self {}
}

/** @template TBuilt of object */
class Builder
{
    /** @return Doubled&TBuilt */
    public function getMock(): Doubled {}
}

class Cases
{
    /**
     * @template T of object
     *
     * @param class-string<T> $type
     *
     * @return Doubled&T
     */
    public function createMock(string $type): Doubled {}

    /**
     * @template T of object
     *
     * @param class-string<T> $type
     *
     * @return Builder<T>
     */
    public function builderFor(string $type): Builder {}

    public function fromTheArgument(): void
    {
        $mock = $this->createMock(Real::class);
        $mo@argument@ck;
    }

    public function fromTheBuilder(): void
    {
        $mock = $this->builderFor(Real::class)->getMock();
        $mo@builder@ck;
    }
}
"#;

/// Both bindings, because the two readers are separate and each was blind to the intersection on its
/// own side.
#[test]
fn a_mock_is_the_class_it_doubles_and_the_double_itself() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("the template is bound by an argument", "argument"),
            Query::variable_type("and by the receiver's own argument", "builder"),
        ],
        expect![[r#"
            the template is bound by an argument
            - Doubled&Real
            and by the receiver's own argument
            - Doubled&Real
        "#]],
    );
}
