//! A method declared to return `self` or `static` handing back a class that is never one.
//!
//! The judgement answers `Unknown` for a relative type, since what it stands for is the class the
//! code is written in. The return check knows that class, so a value whose class refuses it -- an
//! unrelated final class, a sibling -- is reported, and everything that may be an instance of it
//! is not.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn only_a_class_that_can_never_be_the_enclosing_one_is_reported() {
    check(
        r#"
//- /src/Builder.php
<?php
namespace App;

interface Buildable {}

final class Other {}

class Builder implements Buildable
{
    public function sibling(): static
    {
        return new Other();
    }

    public function itself(): self
    {
        return new Other();
    }

    public function fresh(): static
    {
        return new static();
    }

    public function plain(): self
    {
        return new self();
    }

    public function same(): static
    {
        return $this;
    }

    public function below(): self
    {
        return new Special();
    }

    public function anyBuildable(Buildable $built): static
    {
        return $built;
    }
}

class Special extends Builder {}
"#,
        expect![[r#"
            12:16-12:27 error [incompatible-return] Cannot return Other where static is declared
            17:16-17:27 error [incompatible-return] Cannot return Other where self is declared
        "#]],
    );
}
