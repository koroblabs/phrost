//! A named argument is judged like a positional one, because PHP fills the same parameter with it.
//!
//! Three type-judging walks each opened with `if argument.name.is_some() { break; }`, so every
//! check about what a call *passes* went blind at the first `name:` -- and at everything after it,
//! which on a call written entirely by name is the whole call. Laravel's `ListenCommand` builds a
//! `ListenerOptions` out of nine named arguments and nothing was said about any of them.
//!
//! The pairing itself is not hard: positional arguments fill parameters in order, a named one fills
//! the parameter it names, and PHP's grammar forbids a positional argument after a named one, so
//! the two never have to be reconciled. What it declines is an absence of knowledge -- a spread,
//! and a name no parameter has.

use expect_test::expect;

use super::diagnostics::check;

/// A callee with parameters of three different types, and something that hands back a `false`.
///
/// Declared here rather than reached for: the harness indexes no standard library, so a fixture
/// written with `realpath()` tests `undefined-function` and nothing else.
const CALLEE: &str = r#"
//- /src/Callee.php
<?php

class Paths
{
    public function resolve(string $path): string|false
    {
        return $path;
    }
}

class Store
{
    public function put(string $key, int $size, string $body): void
    {
    }
}
"#;

#[test]
fn a_named_argument_is_judged_against_the_parameter_it_names() {
    check(
        &format!(
            r#"{CALLEE}
//- /src/app.php
<?php

function write(Store $store, Paths $paths, string $where): void
{{
    $store->put(key: 'a', size: 1, body: $paths->resolve($where));
}}
"#
        ),
        expect![[r#"
            5:36-5:65 warning [possibly-false-argument] $body takes string, and this may be false
        "#]],
    );
}

#[test]
fn a_named_argument_out_of_declared_order_is_paired_by_name_and_not_by_position() {
    // The case position cannot answer at all: written third, filling the first parameter. Paired
    // positionally it would be judged against `$body`, which happens to take the same type here and
    // would have made a wrong rule look right -- so the subject is `$size`, which does not.
    check(
        &format!(
            r#"{CALLEE}
//- /src/app.php
<?php

function write(Store $store, Paths $paths, string $where): void
{{
    $store->put(body: 'b', key: 'a', size: $paths->resolve($where));
}}
"#
        ),
        expect![[r#"
            5:38-5:67 warning [possibly-false-argument] $size takes int, and this may be false
        "#]],
    );
}

#[test]
fn a_positional_argument_before_a_named_one_still_fills_its_own_position() {
    check(
        &format!(
            r#"{CALLEE}
//- /src/app.php
<?php

function write(Store $store, Paths $paths, string $where): void
{{
    $store->put($paths->resolve($where), body: 'b', size: 1);
}}
"#
        ),
        expect![[r#"
            5:17-5:40 warning [possibly-false-argument] $key takes string, and this may be false
        "#]],
    );
}

#[test]
fn a_name_no_parameter_has_says_nothing_and_does_not_stop_the_names_after_it() {
    // PHP collects an unknown name into a variadic or raises an `Error` for it, and neither is this
    // check's business. What matters is that it does not take the rest of the call down with it:
    // `body:` is still judged.
    check(
        &format!(
            r#"{CALLEE}
//- /src/app.php
<?php

function write(Store $store, Paths $paths, string $where): void
{{
    $store->put(key: 'a', size: 1, nope: 'x', body: $paths->resolve($where));
}}
"#
        ),
        expect![[r#"
            5:36-5:40 error [unknown-named-parameter] Unknown named parameter $nope
            5:47-5:76 warning [possibly-false-argument] $body takes string, and this may be false
        "#]],
    );
}
