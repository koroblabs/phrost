//! WordPress's hash notation: `@return array { Description. @type T $key Description. }`.
//!
//! mago reads `array {` as a shape with a space in it and fails on the prose inside, so the tag said
//! nothing -- not even `array`. The docblock is rewritten to its own length first, braces to spaces
//! and `@type` to `@var `, so what comes back is a shape built from the keys and every span is still
//! a span of the file; the last test is about that half.
//!
//! The fixtures declare no native return hint, because `check_hir` renders a docblock's return only
//! where the signature carries none.

use expect_test::expect;

use super::{check_hir, check_name_refs};

#[test]
fn the_keys_a_hash_lists_are_the_shape_of_what_comes_back() {
    check_hir(
        r#"
//- /src/app.php
<?php

/**
 * @return array {
 *     Values for the extension and mime type.
 *
 *     @type string|false $ext  File extension, or false if the file doesn't match a mime type.
 *     @type string|false $type File mime type, or false if the file doesn't match a mime type.
 * }
 */
function check_filetype( $filename ) {}

/**
 * @return int[] {
 *     An array of width and height values.
 *
 *     @type int \$0 The maximum width in pixels.
 *     @type int $1 The maximum height in pixels.
 * }
 */
function constrain( $width, $height ) {}

/**
 * @return array|false {
 *     Array containing the host and the port, or false.
 *
 *     @type string   \$0 Host name.
 *     @type int|null $1 Port.
 * }
 */
function parse_host( $host ) {}

/**
 * @return array {
 *     The block's styles.
 *
 *     @type string $css         A CSS ruleset.
 *     @type array  $declarations {
 *         Each declaration.
 *
 *         @type string $name The property.
 *     }
 *     @type string $classnames  Class names.
 * }
 */
function get_styles( $styles ) {}
"#,
        expect![[r#"
            function check_filetype($filename) doc: array{ext: string|false, type: string|false, ...} @ 11:10-11:24  [doc]
            function constrain($width, $height) doc: array{0: int, 1: int, ...} @ 21:10-21:19  [doc]
            function parse_host($host) doc: array{0: string, 1: int|null, ...}|false @ 31:10-31:20  [doc]
            function get_styles($styles) doc: array{css: string, declarations: array{name: string, ...}, classnames: string, ...} @ 46:10-46:20  [doc]
        "#]],
    );
}

/// A brace that is not a hash: one that never closes, one on a tag that is not a return, and a
/// PHPStan shape written across lines -- with or without a space, since it holds no `@type`.
#[test]
fn a_brace_that_is_not_a_hash_is_left_alone() {
    check_hir(
        r#"
//- /src/app.php
<?php

/**
 * @param array $args {
 *     Optional. Arguments.
 *
 *     @type int $number How many.
 * }
 * @return string
 */
function a_parameter_hash( $args ) {}

/**
 * @return array {
 *     Never closed.
 */
function an_open_brace() {}

/**
 * @return array{
 *     date: string,
 *     rate: float,
 * }
 */
function a_shape_across_lines() {}

/**
 * @return array {
 *     date: string,
 * }
 */
function a_spaced_shape_across_lines() {}
"#,
        expect![[r#"
            function a_parameter_hash($args) doc: string @ 11:10-11:26  [doc]
            function an_open_brace() @ 17:10-17:23  [doc]
            function a_shape_across_lines() doc: array{date: string, rate: float} @ 25:10-25:30  [doc]
            function a_spaced_shape_across_lines() doc: array{date: string} @ 32:10-32:37  [doc]
        "#]],
    );
}

/// A class a key names keeps the range it was written at, because the rewrite moved nothing.
#[test]
fn a_class_a_key_names_is_where_it_was_written() {
    check_name_refs(
        r#"
//- /src/app.php
<?php

/**
 * @return array {
 *     @type WP_Post $post The post.
 * }
 */
function first_post() {}
"#,
        expect![[r#"
            WP_Post  [ClassLike] @ 5:14-5:21  [doc]
        "#]],
    );
}
