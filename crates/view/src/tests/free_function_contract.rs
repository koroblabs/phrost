//! A free function's `@template`, which used to be read only when written as a conditional.
//!
//! `returned_by_function_condition` was named for what it did: it read a `@return ($p is T ? A : B)`
//! and nothing else. So a free function's `@template T` + `@param class-string<T>` + `@return T` --
//! the factory shape -- answered whatever the native hint said, which for a factory is `object` and
//! for one with no hint is nothing at all. A method one line away answered the class.
//!
//! For a framework whose `.module`, `.inc` and `.install` files are free functions by definition,
//! that is not an edge: every factory a project moves out of a class loses its template.
//!
//! Four spellings, because the hint was the thing suspected first and is not the thing: `object`,
//! `mixed` and none all answer the same now, and so does a call written with the namespace spelled
//! out.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_free_function_binds_its_template_whatever_its_hint_says() {
    check_view(
        r#"
//- /src/helpers.php
<?php
namespace App;

class Row
{
    public function label(): string
    {
        return '';
    }
}

/**
 * @template T of object
 * @param class-string<T> $name
 * @return T
 */
function make_object(string $name): object {}

/**
 * @template T
 * @param class-string<T> $name
 * @return T
 */
function make_loose(string $name): mixed {}

/**
 * @template T
 * @param class-string<T> $name
 * @return T
 */
function make_bare(string $name) {}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): void
    {
        $hinted = make_object(Row::class);
        $hi@hinted@nted;
        $loose = make_loose(Row::class);
        $lo@loose@ose;
        $bare = make_bare(Row::class);
        $ba@bare@re;
        $qualified = \App\make_object(Row::class);
        $qu@qualified@alified;
        $unsettled = make_object($runtime);
        $un@unsettled@settled;
    }
}
"#,
        &[
            Query::variable_type("an object hint", "hinted"),
            Query::variable_type("a mixed hint", "loose"),
            Query::variable_type("no hint at all", "bare"),
            Query::variable_type("written out in full", "qualified"),
            Query::variable_type("a name nothing settles", "unsettled"),
        ],
        expect![[r#"
            an object hint
            - Row
            a mixed hint
            - Row
            no hint at all
            - Row
            written out in full
            - Row
            a name nothing settles
            - object
        "#]],
    );
}
