//! A `bool` handed to a `true` or a `false`, and a scalar handed to either.
//!
//! `bool` is `true|false`, so against one half it is a union with a branch that cannot land -- the
//! argument check reports `int|string` against an `int` for the same reason, and this answered
//! "unknown" instead. Weak mode does not save it: run on PHP 8.5.10 without `strict_types`, `1`,
//! `1.0` and `"x"` passed to a declared `true` are each a `TypeError`, where a declared `bool` takes
//! all three. A guard that names the half settles it first, in either polarity.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_bool_is_one_half_only_where_a_guard_said_which() {
    check(
        r#"
//- /src/Flags.php
<?php
namespace App;

function takesFalse(false $value): void {}

function takesTrue(true $value): void {}

function run(bool $flag, int $count): void
{
    takesFalse($flag);
    takesTrue($count);
    if ($flag === false) {
        takesFalse($flag);
    }
    if ($flag !== false) {
        takesTrue($flag);
    }
    if (!$flag) {
        takesFalse($flag);
    }
}
"#,
        expect![[r#"
            10:16-10:21 error [incompatible-argument] $value takes false, and this is bool
            11:15-11:21 error [incompatible-argument] $value takes true, and this is int
        "#]],
    );
}
