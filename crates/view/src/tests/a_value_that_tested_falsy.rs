//! What a value can still be where it tested falsy.
//!
//! `if (!$x)` used to answer only where null was all that was left -- `?Repo` -- and declined
//! everywhere else, which took the whole type out of the branch. Beside an `is_wp_error()` in an
//! `||`, the falsy half was then nothing and so was the union of the two, and every
//! `$tag->get_error_message()` WordPress writes under `if (!$tag || is_wp_error($tag))` resolved
//! nowhere. An object, a `true` and a shape that lists a key it always has cannot be falsy; a `bool`
//! is its `false`; an array is the empty one; everything else stays as it was.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_falsy_test_takes_off_what_is_always_truthy() {
    check_view(
        r#"
//- /src/app.php
<?php
class WP_Error {}
class WP_Term {}

/**
 * @phpstan-assert-if-true WP_Error $thing
 */
function is_wp_error( $thing ): bool {}

/** @return array|WP_Error|null */
function lookup() {}
/** @return WP_Term|array{term_id: int}|WP_Error|null */
function term() {}

function run( ?int $count, ?bool $flag ) {
    $tag = lookup();
    if ( ! $tag || is_wp_error( $tag ) ) {
        $@tag@tag;
    }
    $term = term();
    if ( ! $term ) {
        $@term@term;
    }
    if ( ! $count ) {
        $@count@count;
    }
    if ( ! $flag ) {
        $@flag@flag;
    }
}
"#,
        &[
            Query::variable_type("falsy, or the error the assertion names", "tag"),
            Query::variable_type("objects and a shape with a key go", "term"),
            Query::variable_type("zero is falsy, so the int stays", "count"),
            Query::variable_type("a bool is its false", "flag"),
        ],
        expect![[r#"
            falsy, or the error the assertion names
            - array{}|WP_Error|null
            objects and a shape with a key go
            - null
            zero is falsy, so the int stays
            - ?int
            a bool is its false
            - false|null
        "#]],
    );
}
