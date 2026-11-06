//! `$x === 'draft'` proves `$x` is not null, and nothing said so.
//!
//! [`CompareOp::null_holds_whatever_it_faces`] answers the relational half -- `null > x` is false
//! for every operand and `null <= x` is true for every one -- and the equality operators answer
//! `None` there, because whether null satisfies them depends on what they face. That was read as
//! "nothing to say" rather than as "ask the operand", so a guard comparing a nullable against a
//! literal narrowed nothing at all and every use below it was reported.
//!
//! Only the strict spellings. Measured on PHP 8.5.10: `null == 0`, `null == ''` and `null == []`
//! all hold, so `$x == 0` says nothing about null -- while `null === 0` is false for every literal
//! there is.
//!
//! And only the operands whose *spelling* settles it: a literal that is not `null`, an array, a
//! `new`, and an operator whose result the operator itself decides. A class constant is the
//! interesting refusal -- `Stage::APPROVED` is an enum case and can never be null, but a constant
//! is a name this layer cannot resolve and `const X = NULL;` is legal. Measured across the five
//! corpora it is worth one finding, which is not what an unsound claim costs.

use expect_test::expect;

use super::diagnostics::check;

/// A nullable in four guards, three of which say the same thing and one of which does not.
const GUARDS: &str = r#"
//- /src/Guards.php
<?php

class Guards
{
    public function text(): ?string
    {
        return NULL;
    }

    public function takes(string $value): string
    {
        return $value;
    }
}
"#;

#[test]
fn a_strict_identity_against_a_literal_rules_the_null_out() {
    check(
        &format!(
            r#"{GUARDS}
//- /src/app.php
<?php

function run(Guards $guards): string
{{
    $value = $guards->text();

    if ($value === 'draft') {{
        return $guards->takes($value);
    }}

    return '';
}}
"#
        ),
        expect![""],
    );
}

#[test]
fn the_branch_the_identity_failed_on_proves_nothing() {
    // The half that must stay silent: the else of `$value === 'draft'` holds null as readily as
    // 'sent', so the finding there is the one the rule exists to keep.
    check(
        &format!(
            r#"{GUARDS}
//- /src/app.php
<?php

function run(Guards $guards): string
{{
    $value = $guards->text();

    if ($value !== 'draft') {{
        return $guards->takes($value);
    }}

    return '';
}}
"#
        ),
        expect![[r#"
            8:31-8:37 warning [possibly-null-argument] $value takes string, and this may be null
        "#]],
    );
}

#[test]
fn a_loose_comparison_says_nothing_because_php_says_nothing() {
    // `null == ''` holds, so the guard is satisfied by the very value it looks like it rules out.
    check(
        &format!(
            r#"{GUARDS}
//- /src/app.php
<?php

function run(Guards $guards): string
{{
    $value = $guards->text();

    if ($value == '') {{
        return $guards->takes($value);
    }}

    return '';
}}
"#
        ),
        expect![[r#"
            8:31-8:37 warning [possibly-null-argument] $value takes string, and this may be null
        "#]],
    );
}
