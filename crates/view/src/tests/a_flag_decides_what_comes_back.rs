//! A conditional return type that turns on a *number*, and a call that writes the constant naming
//! it.
//!
//! `pathinfo($path, PATHINFO_EXTENSION)` hands back a string where the same call without the flag
//! hands back an array, and the stub has always said so: `($flags is 15 ? array{...} : string)`.
//! Nothing read it -- a test against a number was not one of the shapes, and the number is written
//! as a constant besides -- so every one of those calls answered with the bare `array|string` its
//! signature promises, and `strtolower(pathinfo($p, PATHINFO_EXTENSION))` reported an array being
//! passed where a string was wanted.
//!
//! The constant is resolved rather than pattern-matched: what `PATHINFO_EXTENSION` stands for is in
//! whichever file declares it, which is exactly the sort of thing the index is for.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_number_a_constant_names_settles_the_branch() {
    check_view(
        r#"
//- /stubs/standard.php  stub
<?php
const PATHINFO_EXTENSION = 4;
const PATHINFO_ALL = 15;
/**
 * @param 1|2|4|8|15 $flags
 *
 * @return ($flags is 15 ? array{basename: string} : string)
 */
function pathinfo(string $path, int $flags = PATHINFO_ALL): array|string {}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(string $path): void
    {
        $e@named@xtension = pathinfo($path, PATHINFO_EXTENSION);
        $n@number@umber = pathinfo($path, 4);
        $a@all@ll = pathinfo($path, PATHINFO_ALL);
        $p@plain@lain = pathinfo($path);
    }
}
"#,
        &[
            Query::variable_type("the flag as its constant", "named"),
            Query::variable_type("the number the constant stands for", "number"),
            Query::variable_type("the flag the else branch is written for", "all"),
            Query::variable_type("no flag at all", "plain"),
        ],
        // The last one is the half this leaves open, and it is a miss rather than a wrong answer:
        // the argument is absent, so what decides is the *default* -- and that is written as a
        // constant too, which nothing at lowering can resolve. Both branches stand.
        expect![[r#"
            the flag as its constant
            - string
            the number the constant stands for
            - string
            the flag the else branch is written for
            - array{basename: string}
            no flag at all
            - array{basename: string}|string
        "#]],
    );
}

/// `wp_parse_url()` tests its component against `-1`, which a docblock writes as a minus in front of
/// a number, and defaults it to the same -- so a call that leaves the component out wants the whole
/// URL. The number has to be read on both sides: in the test, and in the default the declaration
/// writes.
#[test]
fn a_negative_number_and_a_written_default_settle_the_branch() {
    check_view(
        r#"
//- /stubs/standard.php  stub
<?php
const PHP_URL_HOST = 1;
const PHP_URL_PORT = 2;

//- /src/http.php
<?php
/**
 * @phpstan-return ($component is -1 ? false|array{host?: string} : ($component is 2 ? int|null : string|null))
 */
function wp_parse_url(string $url, int $component = -1) {}

/**
 * @return ($flags is 4 ? string : int)
 */
function flagged(int $flags = 4) {}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(string $url, int $component): void
    {
        $w@whole@hole = wp_parse_url($url);
        $h@host@ost = wp_parse_url($url, PHP_URL_HOST);
        $p@port@ort = wp_parse_url($url, PHP_URL_PORT);
        $u@unknown@nknown = wp_parse_url($url, $component);
        $f@defaulted@lagged = flagged();
    }
}
"#,
        &[
            Query::variable_type("no component, so the default of -1", "whole"),
            Query::variable_type("a component that is not -1", "host"),
            Query::variable_type("the component the inner test names", "port"),
            Query::variable_type("a component nothing settles", "unknown"),
            Query::variable_type("a positive number written as the default", "defaulted"),
        ],
        expect![[r#"
            no component, so the default of -1
            - false|array{host?: string}
            a component that is not -1
            - ?string
            the component the inner test names
            - ?int
            a component nothing settles
            - false|array{host?: string}|int|string|null
            a positive number written as the default
            - string
        "#]],
    );
}
