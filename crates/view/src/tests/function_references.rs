//! Which function names the file writes, and what each one resolves to.
//!
//! The layer that owns PHP's two function-lookup rules, and the one where getting either wrong is
//! cheapest to see: an unqualified name inside a namespace falls back to the global table, and a
//! `use function` import supplies a prefix. A projection that missed the first would resolve nothing
//! in a namespaced file, and every `count()` in it would read as undeclared.
//!
//! Every fixture carries at least one name that resolves and one that does not, so a snapshot is
//! never uniform by accident -- an all-unresolved answer would otherwise be indistinguishable from
//! the projection declining to resolve at all.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};

use crate::IndexView;

/// Every function name in source order, with what it resolved to.
fn check(fixture: &str, expect: Expect) {
    let world = World::parse(fixture);
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let found = view
            .function_references()
            .iter()
            .map(|reference| {
                format!(
                    "{} {} -> {}\n",
                    world.render_range(file, reference.range),
                    reference.path,
                    reference
                        .resolved
                        .as_ref()
                        .map_or("<nothing>", |fqn| fqn.as_str()),
                )
            })
            .collect::<String>();

        if found.is_empty() {
            "<no function names>\n".to_owned()
        } else {
            found
        }
    });

    expect.assert_eq(&rendered);
}

#[test]
fn an_unqualified_name_in_a_namespace_falls_back_to_the_global_table() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function strlen(string $value): int {}
function array_map(callable $callback, array $array): array {}

//- /app.php
<?php
namespace App;

function run(string $text, array $rows): void
{
    strlen($text);
    array_map(null, $rows);
    array_mapp($rows);
}
"#,
        expect![[r#"
            6:5-6:11 strlen -> strlen
            7:5-7:14 array_map -> array_map
            8:5-8:15 array_mapp -> <nothing>
        "#]],
    );
}

#[test]
fn a_name_declared_in_the_current_namespace_wins_over_the_global_one() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function shout(string $value): string {}

//- /src/local.php
<?php
namespace App;
function shout(string $value): string {}

//- /app.php
<?php
namespace App;

function run(string $text): void
{
    shout($text);
}
"#,
        expect![[r#"
            6:5-6:10 shout -> App\shout
        "#]],
    );
}

#[test]
fn an_imported_function_resolves_and_so_does_an_aliased_one() {
    check(
        r#"
//- /src/helpers.php
<?php
namespace App\Helpers;
function shout(string $value): string {}
function whisper(string $value): string {}

//- /app.php
<?php
namespace Other;

use function App\Helpers\shout;
use function App\Helpers\whisper as quietly;
use function App\Helpers\missing as absent;

function run(string $text): void
{
    shout($text);
    quietly($text);
    absent($text);
}
"#,
        expect![[r#"
            10:5-10:10 shout -> App\Helpers\shout
            11:5-11:12 quietly -> App\Helpers\whisper
            12:5-12:11 absent -> <nothing>
        "#]],
    );
}

#[test]
fn a_fully_qualified_name_ignores_the_namespace_it_sits_in() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function array_keys(array $array): array {}

//- /app.php
<?php
namespace App;

function run(array $rows): void
{
    \array_keys($rows);
    \App\array_keys($rows);
}
"#,
        expect![[r#"
            6:5-6:16 array_keys -> array_keys
            7:5-7:20 App\array_keys -> <nothing>
        "#]],
    );
}

#[test]
fn a_value_being_called_is_not_a_function_name() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function strlen(string $value): int {}
function array_map(callable $callback, array $array): array {}
function call_user_func(callable $callback): mixed {}

//- /app.php
<?php
namespace App;

function run(string $text, array $rows): void
{
    $f = 'nope_not_declared';
    $f();
    ($f)();
    array_map('nope_not_declared', $rows);
    call_user_func('nope_not_declared');
    strlen($text);
}
"#,
        expect![[r#"
            9:5-9:14 array_map -> array_map
            10:5-10:19 call_user_func -> call_user_func
            11:5-11:11 strlen -> strlen
        "#]],
    );
}

#[test]
fn a_first_class_callable_is_a_name_even_though_it_is_not_a_call() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function strlen(string $value): int {}

//- /app.php
<?php
namespace App;

function run(): void
{
    $good = strlen(...);
    $bad = strlenn(...);
}
"#,
        expect![[r#"
            6:13-6:19 strlen -> strlen
            7:12-7:19 strlenn -> <nothing>
        "#]],
    );
}
