//! Which parameter a *named* argument is filling.
//!
//! The name says it outright, which is the whole reason somebody writes one -- and reading the
//! argument's index instead pointed `find(tag: 'x')` at the first parameter and `find(id: 1, tag:
//! 'x')` at none. A highlighted parameter is read as a statement about the argument being typed, so
//! being wrong about it is worse than saying nothing, and here there was nothing to be unsure of.
//!
//! The positional rule beside it is unchanged and still refuses: PHP allows nothing positional
//! after a named argument, so a call being written into that state has no parameter at that index.
//! See `past_a_named_argument_no_parameter_is_marked` in `signature`.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_name_says_which_parameter_is_being_filled_wherever_the_argument_sits() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id, bool $strict = false, ?string $tag = null): void {}

    public function fromTheFirstPosition(): void
    {
        $this->find(tag: 'x'@skipping@);
    }

    public function afterAPositionalOne(): void
    {
        $this->find(1, tag: 'x'@past@);
    }

    public function afterAnotherNamedOne(): void
    {
        $this->find(strict: true, tag: 'x'@second@);
    }

    public function withNothingTypedYet(): void
    {
        $this->find(strict: @empty@);
    }

    public function underTheNameItself(): void
    {
        $this->find(t@onTheName@ag: 'x');
    }

    public function aNameNothingAnswersTo(): void
    {
        $this->find(tga: 'x'@misspelt@);
    }
}
"#,
        &[
            Query::signature("a name skipping two parameters", "skipping"),
            Query::signature("a name past a positional argument", "past"),
            Query::signature("a name past another named one", "second"),
            Query::signature("the value not typed yet", "empty"),
            Query::signature("the cursor on the name itself", "onTheName"),
            Query::signature("a name no parameter answers to", "misspelt"),
        ],
        expect![[r#"
            a name skipping two parameters
            - find(int $id, bool $strict = false, ?string $tag = null)
                int $id
                bool $strict = false
              > ?string $tag = null
            a name past a positional argument
            - find(int $id, bool $strict = false, ?string $tag = null)
                int $id
                bool $strict = false
              > ?string $tag = null
            a name past another named one
            - find(int $id, bool $strict = false, ?string $tag = null)
                int $id
                bool $strict = false
              > ?string $tag = null
            the value not typed yet
            - find(int $id, bool $strict = false, ?string $tag = null)
                int $id
              > bool $strict = false
                ?string $tag = null
            the cursor on the name itself
            - find(int $id, bool $strict = false, ?string $tag = null)
                int $id
                bool $strict = false
              > ?string $tag = null
            a name no parameter answers to
            - find(int $id, bool $strict = false, ?string $tag = null)
                int $id
                bool $strict = false
                ?string $tag = null
              (no parameter is being filled)
        "#]],
    );
}
