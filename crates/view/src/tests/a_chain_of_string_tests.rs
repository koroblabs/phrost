//! A return type that picks a branch by which string an argument is.
//!
//! WordPress's getters: `get_post($post, $output = OBJECT)` is `@phpstan-return ($output is 'ARRAY_A'
//! ? array : ($output is 'ARRAY_N' ? array : WP_Post|null))`. A conditional carries one test, so a
//! chain of them had nowhere to go and the function declared nothing at all -- and `OBJECT` is a
//! constant `define()` declares in another file, so the default has to be read where it is.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_string_the_call_passes_or_defaults_to_picks_the_branch() {
    check_view(
        r#"
//- /src/constants.php
<?php
define( 'OBJECT', 'OBJECT' );
define( 'ARRAY_A', 'ARRAY_A' );
const ARRAY_N = 'ARRAY_N';

//- /src/post.php
<?php
class WP_Post {}

/**
 * @return WP_Post|array|null
 * @phpstan-return (
 *     $output is 'ARRAY_A' ? array<string, mixed>|null : (
 *         $output is 'ARRAY_N' ? array<int, mixed>|null : (
 *             WP_Post|null
 *         )
 *     )
 * )
 */
function get_post( $post = null, $output = OBJECT ) {}

function run(int $id, string $shape): void
{
    $a = get_post( $id );
    $@a@a;
    $b = get_post( $id, ARRAY_A );
    $@b@b;
    $c = get_post( $id, ARRAY_N );
    $@c@c;
    $d = get_post( $id, 'OBJECT' );
    $@d@d;
    $e = get_post( $id, $shape );
    $@e@e;
}
"#,
        &[
            Query::variable_type("the default, a constant declared by define()", "a"),
            Query::variable_type("a constant declared by define()", "b"),
            Query::variable_type("a constant declared by const", "c"),
            Query::variable_type("a string written out", "d"),
            Query::variable_type("a variable, which settles none of the tests", "e"),
        ],
        expect![[r#"
            the default, a constant declared by define()
            - ?WP_Post
            a constant declared by define()
            - ?array<string, mixed>
            a constant declared by const
            - ?array<int, mixed>
            a string written out
            - ?WP_Post
            a variable, which settles none of the tests
            - array<string, mixed>|array<int, mixed>|WP_Post|null
        "#]],
    );
}

/// `get_post_field()` names three fields in one test: `($field is 'ID'|'post_parent'|'menu_order'
/// ? int|'' : ...)`. A union of strings is the same branch reached from each of them, so it is read
/// as one case per string -- and inside a chain of other tests, one link per string.
#[test]
fn a_union_of_strings_is_a_case_for_each() {
    check_view(
        r#"
//- /src/post.php
<?php
/**
 * @phpstan-return (
 *     $field is 'ID'|'post_parent'|'menu_order' ? int : (
 *         $field is 'ancestors' ? int[] : string
 *     )
 * )
 */
function get_post_field( $field, $post = null ) {}

/**
 * @phpstan-return ($args is array{ fields: 'ids', ... } ? int[] : ($kind is 'a'|'b' ? string : float))
 */
function chained( $args, $kind ) {}

function run( $id ) {
    $a = get_post_field( 'post_parent', $id );
    $@a@a;
    $b = get_post_field( 'ancestors', $id );
    $@b@b;
    $c = get_post_field( 'post_status', $id );
    $@c@c;
    $d = chained( array( 'fields' => 'all' ), 'b' );
    $@d@d;
    $e = chained( array( 'fields' => 'all' ), 'c' );
    $@e@e;
}
"#,
        &[
            Query::variable_type("one of the strings the union names", "a"),
            Query::variable_type("the string the next test names", "b"),
            Query::variable_type("a string no test names", "c"),
            Query::variable_type("a union of strings inside a chain", "d"),
            Query::variable_type("a string past it", "e"),
        ],
        expect![[r#"
            one of the strings the union names
            - int
            the string the next test names
            - int[]
            a string no test names
            - string
            a union of strings inside a chain
            - string
            a string past it
            - float
        "#]],
    );
}
