//! A call to a function nothing declares.
//!
//! Every test here is as much about the silence as about the finding. The check exists to catch a
//! typo in a name, and a single wrong report is what teaches a person to stop reading the list --
//! so what a real builtin, an imported function, a value being called and a polyfill guarded by
//! `function_exists` all have to produce is nothing at all.

use expect_test::expect;

use super::diagnostics::check;

/// The stubs a unit fixture needs, kept minimal and synthetic as level 0 requires.
const BUILTINS: &str = r#"
//- /stubs/builtins.php  stub
<?php
function strlen(string $value): int {}
function count(array $value): int {}
function array_map(callable $callback, array $array): array {}
function array_filter(array $array): array {}
function strtolower(string $value): string {}
function call_user_func(callable $callback): mixed {}
function function_exists(string $name): bool {}
"#;

#[test]
fn a_typo_in_a_builtin_and_a_name_nothing_declares_are_both_reported() {
    check(
        &format!(
            r#"{BUILTINS}
//- /app.php
<?php
function run(): void
{{
    array_mapp([], []);
    no_such_global_function_zz();
}}
"#
        ),
        expect![[r#"
            4:5-4:15 error [undefined-function] Undefined function array_mapp()
            5:5-5:31 error [undefined-function] Undefined function no_such_global_function_zz()
        "#]],
    );
}

#[test]
fn a_builtin_called_unqualified_inside_a_namespace_is_not_reported() {
    check(
        &format!(
            r#"{BUILTINS}
//- /app.php
<?php
namespace App\Deep;

function run(string $text, array $rows): void
{{
    strlen($text);
    count($rows);
    array_map(strtolower(...), $rows);
    \strlen($text);
}}
"#
        ),
        expect![[r#""#]],
    );
}

#[test]
fn an_imported_function_an_aliased_one_and_a_neighbour_are_not_reported() {
    check(
        &format!(
            r#"{BUILTINS}
//- /src/helpers.php
<?php
namespace App\Helpers;
function shout(string $value): string {{}}

//- /app.php
<?php
namespace App;

use function App\Helpers\shout;
use function array_filter as keep;

function neighbour(): void {{}}

function run(string $text, array $rows): void
{{
    shout($text);
    keep($rows);
    neighbour();
}}
"#
        ),
        expect![[r#""#]],
    );
}

#[test]
fn a_value_being_called_is_never_reported_however_undeclared_the_string_is() {
    check(
        &format!(
            r#"{BUILTINS}
//- /app.php
<?php
function run(array $rows): void
{{
    $f = 'nope_not_declared';
    $f();
    ($f)();
    array_map('nope_not_declared', $rows);
    call_user_func('nope_not_declared');
}}
"#
        ),
        expect![[r#""#]],
    );
}

#[test]
fn a_name_written_in_a_docblock_is_not_code() {
    check(
        &format!(
            r#"{BUILTINS}
//- /app.php
<?php
/**
 * @see no_such_function_at_all()
 */
function run(): void
{{
    strlen('x');
}}
"#
        ),
        expect![[r#""#]],
    );
}

#[test]
fn a_guarded_polyfill_and_a_nested_declaration_are_both_declarations() {
    // The two shapes PHP does not hoist, and the whole reason this check was off by default. Both
    // really declare the name at runtime, so a call to either is a call to something that exists --
    // the guarded one is how every polyfill package ships, and symfony/var-dumper's `dump()` was
    // the single residual finding on a 1 662-file checkout.
    //
    // What lowering records beside them is that they are conditional, which is what lets them be
    // indexed without shadowing anything: see `conditional_ranking` in `phrost-index` for the half
    // of this that keeps `str_starts_with` pointing at the stub.
    check(
        &format!(
            r#"{BUILTINS}
//- /app.php
<?php
if (!function_exists('guarded')) {{
    function guarded(): void {{}}
}}

function outer(): void
{{
    function nested(): void {{}}
}}

class Service
{{
    public function make(): void
    {{
        function from_a_method(): void {{}}
    }}
}}

switch (PHP_VERSION_ID) {{
    case 80000:
        function in_a_case(): void {{}}
        break;
}}

function run(): void
{{
    guarded();
    nested();
    from_a_method();
    in_a_case();
}}
"#
        ),
        expect![[r#""#]],
    );
}
