//! `is_numeric($x)` names no kind and still says the value is there, and which kinds it is not.
//!
//! The predicate reader answers a `BuiltinTy`, so a test whose truth spans three of them had nothing
//! to say -- and three of PHP's do: `is_numeric` is true for an int, a float and a numeric string,
//! `is_scalar` for those and a bool, `is_countable` for an array and a `Countable`. What each of them
//! settles *and this layer can spell* is that the value is not null, and the kinds no value passing
//! it can be.
//!
//! `strictly`, because none is a truthiness test: `is_numeric('0')` is true and `'0'` is falsy, so
//! reading one as `if ($x)` would rule out a value it admits.
//!
//! Three, and not everything that happens to reject null. A test whose useful half is expressible as
//! a kind belongs in `type_predicate` instead, where it says more.

use expect_test::expect;

use super::check;

#[test]
fn a_numeric_test_leaves_a_value_that_is_there() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $hour): void
    {
        if (is_numeric($hour)) {
            $ho$0ur;
        }
    }
}
"#,
        "hour",
        expect!["none of (null|false|object|array|iterable|callable|bool|resource) $hour"],
    );
}

#[test]
fn the_other_two_say_the_same_thing() {
    check(
        r#"<?php
class Repo
{
    public function run(?array $rows): void
    {
        if (is_countable($rows)) {
            $ro$0ws;
        }
    }
}
"#,
        "rows",
        expect!["none of (null|false|string|int|float|bool|resource) $rows"],
    );
}

/// And the branch where the test failed says nothing: a value that is not numeric is null as readily
/// as it is an object.
#[test]
fn the_other_branch_of_one_proves_nothing() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $hour): void
    {
        if (!is_numeric($hour)) {
            $ho$0ur;
        }
    }
}
"#,
        "hour",
        expect!["?string"],
    );
}
