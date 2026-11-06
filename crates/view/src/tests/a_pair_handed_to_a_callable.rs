//! `call_user_func([$formatter, 'format'])` and its neighbours: a pair of a variable and a method
//! name, handed to a parameter that invokes it, gives back what the method does.
//!
//! The lowering leaves that pair an array, because `[$id, 'asc']` is the same shape holding data;
//! the parameter is what says it is called, and the variable's class is what says which method.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_pair_gives_back_what_its_method_returns() {
    check_view(
        r#"
//- /stubs/standard.php  stub
<?php
/**
 * @template R
 * @param callable(): R $callback
 * @return R
 */
function call_user_func(callable $callback, mixed ...$args): mixed {}

//- /src/app.php
<?php
namespace App;

class Formatter
{
    public function format(): string { return ''; }
}

function run(Formatter $formatter, int $id): void
{
    $made = call_user_func([$formatter, 'format']);
    $none = call_user_func([$formatter, 'missing']);
    $data = call_user_func([$id, 'asc']);
    $m@made@ade;
    $n@none@one;
    $d@data@ata;
}
"#,
        &[
            Query::variable_type("the method the pair names", "made"),
            Query::variable_type("a method the class does not declare", "none"),
            Query::variable_type("a pair of data", "data"),
        ],
        expect![[r#"
            the method the pair names
            - string
            a method the class does not declare
            - mixed
            a pair of data
            - mixed
        "#]],
    );
}

/// The pair held by a variable reads as the pair written in place: one rule, which `$pair()`
/// reaches as well.
#[test]
fn a_pair_in_a_variable_gives_back_what_its_method_returns() {
    check_view(
        r#"
//- /stubs/standard.php  stub
<?php
/**
 * @template R
 * @param callable(): R $callback
 * @return R
 */
function call_user_func(callable $callback, mixed ...$args): mixed {}

//- /src/app.php
<?php
namespace App;

class Formatter
{
    public function format(): string { return ''; }
}

function run(Formatter $formatter): void
{
    $pair = [$formatter, 'format'];
    $made = call_user_func($pair);
    $m@made@ade;
}
"#,
        &[Query::variable_type(
            "the method the held pair names",
            "made",
        )],
        expect![[r#"
            the method the held pair names
            - string
        "#]],
    );
}
