//! A property filled in every arm of an `if` is not the `?T` its declaration says.
//!
//! `if ($c) { $this->x = a(); } else { $this->x = b(); }` is how a value is built lazily when the
//! two ways of building it differ -- Symfony's `Router::getGenerator` writes it, and so does every
//! class that picks an implementation from configuration. The join read it as three possibilities:
//! the two writes, and what the property held before the construct, which is the declared `?T`. So
//! the null came back and every use of the value below reported one that cannot arrive.
//!
//! The guarded shape beside it -- `if ($this->x === NULL) { $this->x = a(); }` -- was already right,
//! and for a different reason: there the guard says what the skipped path holds. This one has no
//! skipped path at all.
//!
//! The `if` with no `else` is here as the control, because the third operand is exactly what it
//! needs: that path really does return whatever the declaration allowed.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn every_arm_writing_leaves_no_null_and_one_arm_writing_still_does() {
    check(
        r#"
//- /src/Build.php
<?php
namespace App;

class Formatter
{
    public function render(): string {}
}

class Runner
{
    private ?Formatter $formatter = NULL;

    public function bothArms(bool $rich): Formatter
    {
        if ($rich) {
            $this->formatter = new Formatter();
        } else {
            $this->formatter = new Formatter();
        }

        return $this->formatter;
    }

    public function oneArm(bool $rich): Formatter
    {
        if ($rich) {
            $this->formatter = new Formatter();
        }

        return $this->formatter;
    }
}
"#,
        expect![[r#"
            30:16-30:32 warning [possibly-null-return] ?Formatter returned where \App\Formatter is declared
        "#]],
    );
}
