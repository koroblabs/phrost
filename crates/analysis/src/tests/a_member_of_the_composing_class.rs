//! `$this->x()` inside a trait, where the composing class declares `x`.
//!
//! A trait is half a class body. `$this` inside one is whoever composed it, PHP resolves the member
//! against that class at runtime, and which class it is a trait cannot name -- so a member missing
//! from the trait is not missing.
//!
//! Reported until now, and Drupal could not show it: its traits mostly declare what they use, so
//! `$this->t()` is in `StringTranslationTrait` itself. Laravel's `View\Concerns\*` are the opposite
//! -- partial class bodies that read `$this->renderedOnce` and call `$this->getPushedContent()`,
//! both declared by the `Factory` that uses them. Believing the trait was **992 of the 2 491
//! findings in Laravel's own `src/`**, and every `undefined-property` in its top six.
//!
//! The cost is stated in `outcome_for` and is real: a typo on `$this->` inside a trait is not
//! caught. Recovering it means analysing the trait once per composing class, which is a scan of
//! every class in the index.

use expect_test::expect;

use super::diagnostics::check;

/// A method and a property, neither declared by the trait, both declared by the class using it.
///
/// The silence here is *unconditional* inside a trait and this fixture cannot say otherwise --
/// sabotaged to `$this->nowhere()`, declared in neither file, it stays silent, which is the cost
/// stated above. What keeps that from being a blanket silence about traits is the third test, where
/// the composing class is still checked; and the second, where the trait's own members resolve.
#[test]
fn a_member_the_composing_class_declares_is_not_missing() {
    check(
        r#"
//- /host.php
<?php
class Host
{
    use ManagesThings;

    public string $label = '';

    public function name(): string
    {
        return 'x';
    }
}

//- /trait.php
<?php
trait ManagesThings
{
    public function manage(): string
    {
        return $this->name() . $this->label;
    }
}
"#,
        expect![""],
    );
}

/// The trait's own members still resolve, so this is not a blanket silence about traits.
#[test]
fn a_trait_still_answers_for_what_it_declares() {
    check(
        r#"
//- /trait.php
<?php
trait ManagesThings
{
    public function manage(): string
    {
        return $this->declaredHere();
    }

    private function declaredHere(): string
    {
        return 'x';
    }

    public function reads(): string
    {
        return $this->declaredHere();
    }
}
"#,
        expect![""],
    );
}

/// A class using the trait is checked as it always was: composing one does not make it open.
#[test]
fn the_class_that_uses_a_trait_is_still_checked() {
    check(
        r#"
//- /trait.php
<?php
trait ManagesThings
{
    public function manage(): string
    {
        return 'x';
    }
}

//- /host.php
<?php
class Host
{
    use ManagesThings;

    public function run(): string
    {
        return $this->manage() . $this->notAnywhere();
    }
}
"#,
        expect![[r#"
            8:41-8:52 error [undefined-method] Undefined method Host::notAnywhere()
        "#]],
    );
}
