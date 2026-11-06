//! The guards that rule the empty string out, read as the claims they are.
//!
//! `non-empty-string` is a type, so a test that proves a string holds something has to say so, or
//! every `if ($name !== '') { f($name); }` reports the call it was written to protect. What each
//! claim does to a type is the view's and is tested there; which guard makes which claim is the
//! tree's, and is tested here.

use expect_test::expect;

use super::check;

/// The strict comparison rules out `''` and nothing else: the null `!== ''` lets through stays.
#[test]
fn a_strict_comparison_against_the_empty_string_rules_only_it_out() {
    check(
        r#"<?php
function f(?string $name): void
{
    if ($name !== '') {
        $$0name;
    }
}
"#,
        "name",
        expect!["not '' $name"],
    );
}

/// The loose one rules out null and `false` beside it, which `null != ''` and `false != ''` fail.
#[test]
fn a_loose_comparison_rules_out_the_sentinels_too() {
    check(
        r#"<?php
function f(?string $name): void
{
    if ('' == $name) {
        return;
    }
    $$0name;
}
"#,
        "name",
        expect!["not null or '' $name"],
    );
}

/// `strlen($s) > 0` is lowered as the length test it is, and failing it is the same claim.
#[test]
fn a_length_that_is_not_zero_is_a_string_that_holds_something() {
    check(
        r#"<?php
function f(?string $name): void
{
    if (strlen($name) > 0) {
        $$0name;
    }
}
"#,
        "name",
        expect!["not null or '' $name"],
    );
}

/// `str_starts_with($x, '$')` that held: the value holds what its needle does.
#[test]
fn a_needle_found_says_what_the_haystack_holds() {
    check(
        r#"<?php
function f(string $expr): void
{
    if (str_starts_with($expr, '$')) {
        $$0expr;
    }
}
"#,
        "expr",
        expect!["not '' $expr"],
    );
}

/// A loose `in_array` over string literals says the needle's strings hold what theirs do.
#[test]
fn a_loose_search_among_strings_reads_the_haystack() {
    check(
        r#"<?php
function f(string $type): void
{
    if (!in_array($type, ['int', 'uuid'])) {
        return;
    }
    $$0type;
}
"#,
        "type",
        expect!["not '' $type"],
    );
}

/// `$x === null || $x === '' || check($x)` failing keeps both of the first two claims under the
/// contract the third may be: re-anchoring the contract on the bare name cut the nest at its outer
/// claim and put the null back.
#[test]
fn a_contract_composed_on_two_claims_keeps_both() {
    check(
        r#"<?php
class V { public function check(string $s): bool { return true; } }
function f(?string $state, V $v): void
{
    if ($state === null || $state === '' || $v->check($state)) {
        return;
    }
    $$0state;
}
"#,
        "state",
        expect!["argument 0 of check(), asserted if false, was not '' not null $state"],
    );
}
