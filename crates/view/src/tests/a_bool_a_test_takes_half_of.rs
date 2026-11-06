//! A `bool` a settled test has taken `false` or `true` out of, or named outright.
//!
//! `bool` is `true|false` written as one type, so the narrowing that drops a `false` operand from
//! `int|false` found nothing to drop and kept the whole of it: `$handler ?: SIG_DFL` over a
//! `bool|resource` still read as "may be false", on the one line written to rule that out. The
//! passing side of `=== false` had the same gap the other way round -- a `bool` that had just been
//! compared equal to `false` was still a `bool`, which a declared `false` refuses.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn ruling_one_value_out_leaves_the_other() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

/** @param bool|resource $handler */
function run($handler, bool $flag): void
{
    $u@union@nion = $handler ?: 0;
    $l@lone@one = $flag ?: 0;
    if ($flag !== false) {
        $t@strict@rue = $flag;
    }
    if ($flag !== true) {
        $f@other@alse = $flag;
    }
}
"#,
        &[
            Query::variable_type("a union holding a bool", "union"),
            Query::variable_type("a bool alone", "lone"),
            Query::variable_type("compared against false", "strict"),
            Query::variable_type("compared against true", "other"),
        ],
        expect![[r#"
            a union holding a bool
            - true|resource|int
            a bool alone
            - true|int
            compared against false
            - true
            compared against true
            - false
        "#]],
    );
}

#[test]
fn naming_one_value_leaves_that_value() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

function run(bool $flag, ?bool $maybe): void
{
    if ($flag === false) {
        $f@false@alse = $flag;
    }
    if ($flag === true) {
        $t@true@rue = $flag;
    }
    if ($maybe === false) {
        $n@named@amed = $maybe;
    }
    if ($maybe !== false) {
        $r@ruled@uled = $maybe;
    }
}
"#,
        &[
            Query::variable_type("compared equal to false", "false"),
            Query::variable_type("compared equal to true", "true"),
            Query::variable_type("a nullable named false", "named"),
            Query::variable_type("a nullable with false ruled out", "ruled"),
        ],
        expect![[r#"
            compared equal to false
            - false
            compared equal to true
            - true
            a nullable named false
            - false
            a nullable with false ruled out
            - true|null
        "#]],
    );
}
