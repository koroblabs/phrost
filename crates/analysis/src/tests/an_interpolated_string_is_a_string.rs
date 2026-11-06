//! `"order.{$field}[0]"` is a string, and it said nothing at all.
//!
//! The three ways PHP writes a string are a literal, a concatenation and an interpolation. The
//! first two produce a `string` -- the literal by its own kind, the concatenation through
//! `ExprData::Operator`, whose whole job is an operator that decides its own result -- and the
//! third was lowered as `Unmodelled`, which says nothing. So every local assigned one was untyped:
//! `$path = "a/$b"; f($path);` was judged by nobody, and `phrost coverage` counted it among the
//! assignments with no type. It is the same node the concatenation already uses.
//!
//! Measured on the reference project: assignments typed **93.9 % -> 94.5 %**, 71 more of 11 439.
//! On the corpora it is worth a handful of findings, and every one is a value that had been
//! invisible rather than a new rule -- `$src = $_POST['src']; if (…) { $src = "http://$src"; }`
//! joins a `string` with the superglobal's `string|array`, and `wp_basename($src)` takes a string.

use expect_test::expect;

use super::diagnostics::check;

/// A parameter no string can satisfy, so the finding is about the type and not about a coercion
/// weak mode would make.
const TAKES: &str = r#"
//- /src/Takes.php
<?php

class Takes
{
    public function only(array $value): void
    {
    }
}
"#;

#[test]
fn the_three_spellings_of_a_string_all_say_string() {
    check(
        &format!(
            r#"{TAKES}
//- /src/app.php
<?php

function run(Takes $takes, string $field): void
{{
    $literal = 'order';
    $interpolated = "order.{{$field}}[0]";
    $concatenated = 'order.' . $field;

    $takes->only($literal);
    $takes->only($interpolated);
    $takes->only($concatenated);
}}
"#
        ),
        expect![[r#"
            9:18-9:26 error [incompatible-argument] $value takes array, and this is non-falsy-string
            10:18-10:31 error [incompatible-argument] $value takes array, and this is non-falsy-string
            11:18-11:31 error [incompatible-argument] $value takes array, and this is non-falsy-string
        "#]],
    );
}

#[test]
fn a_heredoc_says_it_too() {
    // The same node, and the spelling an application reaches for when the text has quotes in it.
    check(
        &format!(
            r#"{TAKES}
//- /src/app.php
<?php

function run(Takes $takes, string $field): void
{{
    $heredoc = <<<TXT
        order.$field
        TXT;

    $takes->only($heredoc);
}}
"#
        ),
        expect![[r#"
            9:18-9:26 error [incompatible-argument] $value takes array, and this is non-falsy-string
        "#]],
    );
}

#[test]
fn what_the_holes_hold_is_still_read_where_it_decides_something() {
    // The one reader that takes an interpolated argument apart: `strtotime("-$i months")` cannot
    // fail for any integer `$i`, and only the text around the hole says so. It matched the node by
    // shape -- `Unmodelled` -- and the note above it asks for exactly the arm this now has.
    check(
        r#"
//- /src/stub.php  stub
<?php

function strtotime(string $datetime, ?int $baseTimestamp = null): int|false {}

//- /src/app.php
<?php

function takes(int $when): void
{
}

function run(int $months, string $unit): void
{
    $counted = strtotime("-$months months");
    $unitFromAValue = strtotime("$months $unit");

    takes($counted);
    takes($unitFromAValue);
}
"#,
        expect![[r#"
            13:11-13:26 warning [possibly-false-argument] $when takes int, and this may be false
        "#]],
    );
}
