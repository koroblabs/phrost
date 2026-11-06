//! A return decided by the shape of the array a call passes, the way WordPress declares its getters.
//!
//! `get_posts()` is `($args is array{ fields: 'ids', ... } ? int[] : ($args is array{ fields:
//! 'id=>parent', ... } ? array<int, int> : WP_Post[]))`. Neither test had a reading, so the whole
//! return was nothing and every `$post->post_title` after one was untyped. Read off the array the
//! call wrote -- or the one its variable was assigned, with what was written into it since -- nearly
//! every call settles it by leaving the key out. `wp_die()` is the same shape, `($args is array{exit:
//! false} ? void : never)`, and settling it is what lets `if (!$tax) { wp_die(); }` rule the
//! `false` out: 103 findings on WordPress were that.

use expect_test::expect;

use crate::tests::{Query, check};

const FIXTURE: &str = r#"
//- /src/posts.php
<?php
namespace App;

class Post { public string $post_title = ''; }

/**
 * @phpstan-return (
 *     $args is array{ fields: 'ids', ... }
 *         ? int[]
 *         : ( $args is array{ fields: 'id=>parent', ... } ? array<int, int> : Post[] )
 * )
 */
function get_posts($args = null) { return []; }

/**
 * @phpstan-return ( $args is array{ exit: false } ? void : never )
 */
function wp_die($message = '', $args = array()) {}

function run(string $search, ?Post $maybe): void
{
    $a@plain@ = get_posts(array('post_type' => 'page'));
    $b@ids@ = get_posts(array('fields' => 'ids'));
    $args = array('post_status' => 'any');
    if ('' !== $search) {
        $args['s'] = $search;
    }
    $c@variable@ = get_posts($args);
    $args['fields'] = $search;
    $d@unknown@ = get_posts($args);
    $e@string@ = get_posts('fields=ids');
    if (!$maybe) {
        wp_die('gone');
    }
    $f@died@ = $maybe;
}
"#;

#[test]
fn the_array_a_call_wrote_settles_the_shape_test() {
    check(
        FIXTURE,
        &[
            Query::hover("the key left out", "plain"),
            Query::hover("the key written", "ids"),
            Query::hover("a variable, written into at another key", "variable"),
            Query::hover(
                "written into at the key with a value nothing names",
                "unknown",
            ),
            Query::hover("a query string is not the array tested", "string"),
            Query::hover("a call that never returns rules the null out", "died"),
        ],
        expect![[r#"
            the key left out
            - code: Post[] $a
            - note: assigned
            the key written
            - code: int[] $b
            - note: assigned
            a variable, written into at another key
            - code: Post[] $c
            - note: assigned
            written into at the key with a value nothing names
            - code: int[]|array<int, int>|Post[] $d
            - note: assigned
            a query string is not the array tested
            - code: Post[] $e
            - note: assigned
            a call that never returns rules the null out
            - code: Post $f
            - note: assigned
        "#]],
    );
}
