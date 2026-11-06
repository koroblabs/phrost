//! `@param-out` on a by-reference parameter reaches the lowered parameter.
//!
//! The tag says what a by-reference parameter leaves in the caller's variable, which is a different
//! type from what it takes: `preg_match`'s third takes `array` and leaves `array<int, string>`. The
//! bundled stubs write it 145 times.
//!
//! At this layer because the reader is three crates away and a tag that is not lowered is inert
//! without failing anything -- which reads as the feature not working rather than as one parameter
//! shape being missed. Both shapes are here for exactly that reason: gating on mago's `ampersand`
//! *token* rather than on `is_reference()` read `preg_match`'s `&$matches` and silently skipped
//! `str_replace`'s `?int &$count`, because a parameter written with a hint does not carry the token
//! where one written without a hint does. Two spellings of one question, and the field beside it in
//! the same function was already asking the other one.
//!
//! `check_hir` renders the written-back type as `-> T` after the parameter, which it did not before:
//! a snapshot that does not show a field cannot test it, and this test is the reason to show it.

use expect_test::expect;

use super::check_hir;

#[test]
fn both_shapes_of_a_by_reference_parameter_carry_the_written_back_type() {
    check_hir(
        r#"
//- /src/app.php
<?php

/** @param-out int $count */
function hinted(string $s, ?int &$count = NULL): string {}

/** @param-out array<int, string> $matches */
function unhinted(string $s, &$matches = []): bool {}

/** A by-value parameter's tag is a mistake in the docblock, not a claim. */
/** @param-out int $ignored */
function byValue(int $ignored): void {}

/**
 * The shape the vendored stubs actually write: `@param` lines for the by-value parameters and
 * the `@param-out` after them. `preg_match` writes the tag first and `str_replace` writes it
 * fourth, which is the difference between the two and is why both are here.
 *
 * @param string|array<string> $search
 * @param string|array<string> $subject
 *
 * @param-out int $count
 *
 * @return ($subject is string ? string : array<string>)
 */
function afterOtherTags(
    array|string $search,
    array|string $subject,
    ?int &$count = NULL,
): array|string {}
"#,
        expect![[r#"
            function hinted(string $s, ?int &$count = ... -> int): string @ 4:10-4:16  [doc]
            function unhinted(string $s, &$matches = ... -> array<int, string>): bool @ 7:10-7:18  [doc]
            function byValue(int $ignored): void @ 11:10-11:17  [doc]
            function afterOtherTags(array|string $search, array|string $subject, ?int &$count = ... -> int): array|string doc: string|string[] @ 25:10-25:24  [doc]
        "#]],
    );
}
