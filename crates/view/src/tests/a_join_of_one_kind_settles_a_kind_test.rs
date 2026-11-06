//! A union whose every alternative is a string is a string, to a conditional that asks.
//!
//! `$s = replace($s)` in one branch and not the other joins a `string` with the `?string` the call
//! returned -- two spellings the union constructor cannot fold into one, so the join stays a union of
//! two strings. A kind test asked of it answered nothing, the conditional `($subject is string ?
//! string : array<string>)` settled neither branch, and the next call read as `string|string[]`: a
//! wrong answer on a value that is never an array, and a false `incompatible-argument` wherever it
//! went next.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_join_of_strings_settles_a_string_test() {
    check_view(
        r#"
//- /stubs/pcre.php  stub
<?php
/**
 * @param string|array<string> $subject
 * @return ($subject is string ? string|null : array<string>|null)
 */
function replace_all(string $pattern, string|array $subject): string|array|null {}

/**
 * @param string|array<string> $subject
 * @return ($subject is string ? string : array<string>)
 */
function swap(string $search, string|array $subject): string|array {}

//- /src/app.php
<?php
namespace App;

function run(string $text, bool $again): void
{
    $text = replace_all('/a/', $text);
    if ($again) {
        $text = swap('b', $text);
    }
    $j@joined@oined = swap('c', $text);
}
"#,
        &[Query::variable_type(
            "a string or a nullable string, handed to a test of the string",
            "joined",
        )],
        expect![[r#"
            a string or a nullable string, handed to a test of the string
            - string
        "#]],
    );
}
