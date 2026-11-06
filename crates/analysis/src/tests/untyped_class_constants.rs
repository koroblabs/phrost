//! A class constant with no written type still has one, read off its value.
//!
//! The declaration side of this is in `phrost_lower`'s `constant_value_types`; what is here is the
//! half a person notices. Typed constants only arrived in PHP 8.3, so before this a call passing
//! `NodeInterface::PUBLISHED` was passing something the index knew nothing about -- and a framework's
//! interfaces, the standard library and the whole of `vendor/` are written that way.
//!
//! The other half of the same behaviour is snapshotted in `constant_values`, which hovers these
//! declarations and still sees the source's own text: a type nobody wrote must not appear in a card
//! that promises to show what is there. Those tests passing unchanged *is* that guarantee.

use expect_test::expect;

use super::diagnostics::check;

/// The point of the whole thing: the value's type is a real type, so the checker can act on it.
///
/// Under `strict_types`, because PHP's weak mode converts freely between the scalars and an `int`
/// passed where a `string` is declared is then not a mistake at all -- which would make a fixture
/// written to show that the `int` *arrived* prove nothing.
#[test]
fn a_constant_typed_by_its_value_is_judged_like_any_other_argument() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

interface NodeInterface
{
    const PUBLISHED = 1;
    const LABEL = 'node';
    const SLOTS = ['a', 'b'];
}

class Caller
{
    public function takesString(string $text): void {}

    public function takesArray(array $items): void {}

    public function run(): void
    {
        $this->takesString(NodeInterface::LABEL);
        $this->takesArray(NodeInterface::SLOTS);
        $this->takesString(NodeInterface::PUBLISHED);
        $this->takesArray(NodeInterface::LABEL);
    }
}
"#,
        expect![[r#"
            21:28-21:52 error [incompatible-argument] $text takes string, and this is int
            22:27-22:47 error [incompatible-argument] $items takes array, and this is non-falsy-string
        "#]],
    );
}

/// A constant on an *interface* was the reported symptom -- "interface constants do not resolve" --
/// but the boundary was never the declaring kind. It is whether a type was written, and an interface
/// simply cannot write one before 8.3 in any code that still supports 8.2.
#[test]
fn the_declaring_kind_makes_no_difference_to_whether_a_value_is_read() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

interface Face
{
    const FROM_INTERFACE = 1;
}

trait Shared
{
    const FROM_TRAIT = 1;
}

abstract class Base
{
    const FROM_ABSTRACT = 1;
}

final class Leaf extends Base
{
    use Shared;

    const FROM_CLASS = 1;

    public function takesString(string $text): void {}

    public function run(): void
    {
        $this->takesString(Face::FROM_INTERFACE);
        $this->takesString(self::FROM_TRAIT);
        $this->takesString(self::FROM_ABSTRACT);
        $this->takesString(self::FROM_CLASS);
    }
}
"#,
        expect![[r#"
            29:28-29:48 error [incompatible-argument] $text takes string, and this is int
            30:28-30:44 error [incompatible-argument] $text takes string, and this is int
            31:28-31:47 error [incompatible-argument] $text takes string, and this is int
            32:28-32:44 error [incompatible-argument] $text takes string, and this is int
        "#]],
    );
}

/// A value whose type the syntax does not settle leaves the constant where it was: unknown, and
/// therefore never a finding. The safe direction, and the reason the read-list is short.
#[test]
fn a_value_the_syntax_does_not_settle_reports_nothing() {
    check(
        r#"
//- /app.php
<?php
class Flags
{
    const A = 1;

    const BOTH = self::A | 2;

    const SUM = self::A + 1;

    public function takesString(string $text): void {}

    public function run(): void
    {
        $this->takesString(self::BOTH);
        $this->takesString(self::SUM);
    }
}
"#,
        expect![""],
    );
}
