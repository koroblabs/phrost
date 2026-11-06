//! `@return ($callback is null ? Optional : ... TReturn)` beside `(callable(TValue): TReturn)|null`.
//!
//! Laravel's `optional()`. A call leaving `$callback` out passes its default, so the test is settled
//! and the call is an `Optional`; a call passing one is what the callback returns. The `TReturn`
//! branch was refused at lowering as a template nothing binds, and the whole contract with it --
//! every `optional($user)` was `mixed`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_left_out_argument_settles_a_conditional_by_its_default() {
    check_view(
        r#"
//- /src/helpers.php
<?php
namespace App;

class Optional {}

/**
 * @template TValue
 * @template TReturn
 * @param TValue $value
 * @param (callable(TValue): TReturn)|null $callback
 * @return ($callback is null ? Optional : ($value is null ? null : TReturn))
 */
function optional($value = null, ?callable $callback = null) {}

/**
 * @param string|null $format
 * @return ($format is null ? int : string)
 */
function stamp($format = null) {}

//- /src/run.php
<?php
namespace App;

function run(object $user): void
{
    $wrapped = optional($user);
    $w@wrapped@rapped;
    $called = optional($user, fn ($u) => 1);
    $c@called@alled;
    $plain = stamp();
    $p@plain@lain;
}
"#,
        &[
            Query::variable_type("the callback left out", "wrapped"),
            Query::variable_type("the callback given", "called"),
            Query::variable_type("an untemplated parameter left out", "plain"),
        ],
        expect![[r#"
            the callback left out
            - Optional
            the callback given
            - int
            an untemplated parameter left out
            - int
        "#]],
    );
}
