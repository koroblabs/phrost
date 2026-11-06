//! `__construct(/** @var Rule[] */ public array $rules)`.
//!
//! A promoted parameter is one declaration PHP reads as two, and a docblock written above it is
//! about the *property*: the constructor's `@param` is the documented spelling and already worked,
//! and this inline one -- which is what Symfony's own attributes are written with -- did not, so the
//! property was a bare `array` and everything downstream of its element type was untyped.
//!
//! Read at the parameter rather than at the class, because that is where it is written. The offset
//! is the whole parameter's and not the variable's: a docblock precedes `public array $rules`, and
//! asked at `$rules` there is `public array ` in between -- the rule §8 records for a method, whose
//! docblock precedes its modifiers.
//!
//! Measured with `undefined-method` on the element, because that is the only instrument that
//! separates "the element is known" from "the element is unknown" -- an unknown receiver is silent
//! and an array argument judged by its elements needs *both* sides to know theirs.

use expect_test::expect;

use super::diagnostics::check;

const RULE: &str = r#"
//- /rule.php
<?php
class Rule
{
    public function name(): string
    {
        return '';
    }
}
"#;

/// The inline docblock types the property, so a call on its element is checked.
#[test]
fn an_inline_var_above_a_promoted_parameter_types_the_property() {
    check(
        &[
            RULE,
            r#"
//- /app.php
<?php
class Holder
{
    public function __construct(
        /** @var Rule[] */
        public array $rules = [],
    ) {
    }

    public function read(): void
    {
        foreach ($this->rules as $rule) {
            $rule->nope();
        }
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            13:20-13:24 error [undefined-method] Undefined method Rule::nope()
        "#]],
    );
}

/// The constructor's own `@param` wins where both are written, which is why this reads second.
#[test]
fn the_constructors_param_tag_still_decides() {
    check(
        &[
            RULE,
            r#"
//- /app.php
<?php
class Holder
{
    /** @param Rule[] $rules */
    public function __construct(
        /** @var string[] */
        public array $rules = [],
    ) {
    }

    public function read(): void
    {
        foreach ($this->rules as $rule) {
            $rule->nope();
        }
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            14:20-14:24 error [undefined-method] Undefined method Rule::nope()
        "#]],
    );
}

/// And a promoted parameter with no docblock of its own says nothing, so nothing is checked.
///
/// The control this file needs: the finding above has to come from the docblock and not from the
/// hint, which is a bare `array` either way.
#[test]
fn a_promoted_parameter_with_no_docblock_types_nothing() {
    check(
        &[
            RULE,
            r#"
//- /app.php
<?php
class Holder
{
    public function __construct(public array $rules = [])
    {
    }

    public function read(): void
    {
        foreach ($this->rules as $rule) {
            $rule->nope();
        }
    }
}
"#,
        ]
        .concat(),
        expect![""],
    );
}
