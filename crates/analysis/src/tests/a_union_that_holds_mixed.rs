//! A union with `mixed` in it, which every sentinel question has to read as `mixed`.
//!
//! `mixed` already holds every value -- PHP refuses to compile `mixed|null` natively for exactly
//! that reason -- and a docblock writes such unions anyway. Taken apart operand by operand, the
//! null beside the `mixed` was a claim of its own: WordPress's `@return mixed|null` on
//! `WP_REST_Request::offsetGet` made every `$request['url']` a possibly-null argument, and Laravel's
//! `Authenticatable|mixed` parameter refused the null its `mixed` takes. A bare `mixed` is judged
//! by none of these checks, and so neither is a union that contains one.
//!
//! The union itself is kept -- `Row|mixed` still names the class a reader wants completed, which
//! `receiver_agreement` pins. It is only the judgements that see it as `mixed`.
//!
//! One file per subject: `check` renders the last file of a fixture.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_value_that_may_be_mixed_or_null_is_not_a_possibly_null_argument() {
    check(
        r#"
//- /src/app.php
<?php

class Request
{
    /** @return mixed|null */
    public function param(string $key)
    {
        return null;
    }

    /** @return ?mixed */
    public function nullable(string $key)
    {
        return null;
    }

    /** @return string|null */
    public function known(string $key)
    {
        return null;
    }
}

function takes(string $value): void
{
}

function run(Request $request): void
{
    takes($request->param('a'));
    takes($request->nullable('a'));
    takes($request->known('a'));
}
"#,
        expect![[r#"
            32:11-32:31 warning [possibly-null-argument] $value takes string, and this may be null
        "#]],
    );
}

#[test]
fn a_parameter_that_may_be_mixed_takes_a_null() {
    check(
        r#"
//- /src/app.php
<?php

interface User
{
}

class Gate
{
    /** @param User|mixed $user */
    public function allows($user): bool
    {
        return true;
    }

    /** @param User|string $user */
    public function strict($user): bool
    {
        return true;
    }
}

function run(Gate $gate): void
{
    $gate->allows(null);
    $gate->strict(null);
}
"#,
        expect![[r#"
            25:19-25:23 error [null-argument] Cannot pass null as $user, which takes \User|string
        "#]],
    );
}
