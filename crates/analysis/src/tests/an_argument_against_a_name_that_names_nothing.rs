//! An argument judged against a parameter type that resolves to no class at all.
//!
//! The other half of `an_argument_against_a_template`, and it needed a rule of its own because the
//! one there is answered by the *receiver*: a class's template is recognised by being in the
//! declaring class's list, and a **free function's own** `@template` has no list to be in. Only a
//! class carries templates in the index, so `wp_unslash`'s `@template T` + `@param T $value`
//! arrived at the judgement as a class called `T` -- rejecting every scalar for not being one,
//! accepting every class because nothing relates to a name that resolves to nothing, and reporting
//! `$value takes \T`, which is a sentence no reader can act on.
//!
//! A class the project genuinely does not declare reads the same way here. That is
//! `undefined-type`'s finding, said once about the declaration, rather than this check's about
//! every call.
//!
//! Measured over three corpora: **355 findings gone, none added** -- 116 on WordPress, all `\T`;
//! 194 on Laravel, mostly `TValue` and `array<K, V>`; 45 on Symfony. The one loss is a `Traversable`
//! whose stub documents its elements with templates, so the whole expectation declines where the
//! bare hint beside it would have been judged.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_name_nothing_declares_is_not_a_type_to_judge_an_argument_against() {
    check(
        r#"
//- /src/Names.php
<?php
namespace App;

class Row {}

/**
 * @template T
 *
 * @param T $value
 * @return T
 */
function passthrough($value) {}

/** @param \App\NoSuchThing $thing */
function missing($thing): void {}

/** @param Row $row */
function real($row): void {}

class Caller
{
    public function run(Row $row): void
    {
        passthrough(1);
        passthrough('a');
        passthrough($row);

        missing(1);

        real($row);
        real(1);
    }
}
"#,
        expect![[r#"
            14:12-14:28 warning [doc-undefined-type] Undefined type App\NoSuchThing in a docblock
            31:14-31:15 error [incompatible-argument] $row takes \App\Row, and this is int
        "#]],
    );
}
