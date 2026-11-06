//! A project's own accessor, bound to a catalog a framework filled.
//!
//! A framework names its own: `$node->get('field_amount')` is bound to the field catalog by the
//! Drupal provider, so the name completes and a typo in it is visible. A project that wraps those
//! accessors loses both halves, and wrapping them is the norm rather than the exception -- one real
//! checkout routes **171** literal field names through four methods of one trait, against 509
//! written against Drupal's own.
//!
//! Nothing below the seam may name a project's class and nothing in the seam may name a customer's,
//! so the fact is stated in `phrost.toml` and installed through the same call a provider's own
//! binding goes through. From there down there is no difference between the two.
//!
//! # Why the binding may name a trait
//!
//! Because that is what makes it short enough to be written. Ancestry walks `uses` as well as
//! `extends` and `implements`, so one entry per wrapper method covers every class that uses the
//! trait -- four lines for a whole checkout instead of one per class.
//!
//! # Two tests, each with its subject last
//!
//! `check_framework` renders the diagnostics of the fixture's **last** file, which the first version
//! of this got wrong: the negative control was last, so the expectation that was accepted was its
//! emptiness rather than the rule's output, and the test read as coverage while proving nothing.

use expect_test::expect;

use super::diagnostics::check_framework;

const WRAPPER: &str = r#"
//- /src/Wrapper.php
<?php
namespace App;

trait EntityHelperTrait
{
    protected function _field(string $fieldName): string
    {
        return $fieldName;
    }
}
"#;

fn registered(world: &mut phrost_harness::World) {
    // Keyed on the trait, which is what a project writes once for every class that uses it.
    world
        .contracts_mut()
        .bind_argument("App\\EntityHelperTrait", "_field", 0, "", "field");
    world
        .contracts_mut()
        .catalog_entry("field", "field_referral", None);
}

#[test]
fn a_binding_on_a_trait_reaches_a_class_that_uses_it() {
    check_framework(
        &format!(
            "{WRAPPER}{}",
            r#"
//- /src/Supplier.php
<?php
namespace App;

class Supplier
{
    use EntityHelperTrait;

    public function registered(): string
    {
        return $this->_field('field_referral');
    }

    public function misspelt(): string
    {
        return $this->_field('field_referal');
    }
}
"#
        ),
        registered,
        expect![[r#"
            15:30-15:45 warning [unknown-catalog-key] No field is registered as `field_referal`. Did you mean `field_referral`?
        "#]],
    );
}

#[test]
fn a_class_that_declares_a_method_of_the_same_name_is_not_judged_by_it() {
    // The half that keeps this from being a match on the method name alone. `_field` is a name any
    // class may declare for anything, and judging one against a catalog it never agreed to is the
    // "sometimes a key" mistake that turns a closed catalog into an error on correct code.
    check_framework(
        &format!(
            "{WRAPPER}{}",
            r#"
//- /src/Unrelated.php
<?php
namespace App;

class Unrelated
{
    protected function _field(string $fieldName): string
    {
        return $fieldName;
    }

    public function same(): string
    {
        return $this->_field('field_referal');
    }
}
"#
        ),
        registered,
        expect![""],
    );
}
