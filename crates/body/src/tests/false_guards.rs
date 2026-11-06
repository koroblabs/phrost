//! What a strict test against `false` proves.
//!
//! The guard PHP's own standard library forces on its callers. `strpos`, `strtotime`,
//! `array_search`, `file_get_contents` and most of the rest report failure by returning `false`
//! beside their real type, and `!== false` is how every caller rules that out. Without reading it,
//! a value stays `T|false` below its own guard and anything judging what is done with it reports
//! the branch the author just handled.
//!
//! Strict only, and the last test says why.

use expect_test::expect;

use super::check;

#[test]
fn a_negated_identity_rules_false_out_inside() {
    check(
        r#"<?php
class Repo
{
    public function run(string $text): void
    {
        $found = strpos($text, 'x');
        if ($found !== false) {
            $fou$0nd;
        }
    }
}
"#,
        "found",
        expect!["not (false) $found"],
    );
}

#[test]
fn an_early_return_on_false_rules_it_out_below() {
    check(
        r#"<?php
class Repo
{
    public function run(string $text): void
    {
        $when = strtotime($text);
        if ($when === false) {
            return;
        }
        $wh$0en;
    }
}
"#,
        "when",
        expect!["not (false) $when"],
    );
}

#[test]
fn yoda_order_reads_the_same() {
    // Generated code writes it, and the null reader already accepts it from either side.
    check(
        r#"<?php
class Repo
{
    public function run(string $text): void
    {
        $found = strpos($text, 'x');
        if (false !== $found) {
            $fou$0nd;
        }
    }
}
"#,
        "found",
        expect!["not (false) $found"],
    );
}

#[test]
fn the_positive_side_says_the_value_is_false() {
    check(
        r#"<?php
class Repo
{
    public function run(string $text): void
    {
        $found = strpos($text, 'x');
        if ($found === false) {
            $fou$0nd;
        }
    }
}
"#,
        "found",
        expect!["(false) $found"],
    );
}

#[test]
fn a_loose_comparison_proves_nothing_about_false() {
    // `0 == false` and `'' == false` both hold, so reading `== false` as an identity would narrow
    // away an `int` or a `string` the value may still be. It stays an ordinary operator, and the
    // truthiness it really tests narrows through `Not` as it always did.
    check(
        r#"<?php
class Repo
{
    public function run(string $text): void
    {
        $found = strpos($text, 'x');
        if ($found != false) {
            $fou$0nd;
        }
    }
}
"#,
        "found",
        expect!["strpos()"],
    );
}
