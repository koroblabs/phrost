//! A call that leaves an argument out, against a test of what *sort* of value it is.
//!
//! `range(1, 12)` leaves its step out, and the stub's last link is `($step is float ? list<float> :
//! list<int>)` -- so the default `$step = 1` decides. Lowering read a string and a keyword default
//! for a kind test and returned before a number, so the link settled nothing, and once the
//! conditional stood for its branches every `range()` of ints answered the union of all six.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_number_written_as_the_default_is_its_own_kind() {
    check_view(
        r#"
//- /src/range.php
<?php
/**
 * @return (
 *   $start is float
 *   ? list<float>
 *   : ($end is float ? list<float> : ($step is float ? list<float> : list<int>))
 * )
 */
function sequence( int|float $start, int|float $end, int|float $step = 1 ): array {}

/**
 * @return ($step is int ? int : float)
 */
function stepped( int|float $step = -0.5 ) {}

function run( int $i ) {
    $a = sequence( 1, 12 );
    $@a@a;
    $b = sequence( 1, 12, 0.5 );
    $@b@b;
    $c = stepped();
    $@c@c;
}
"#,
        &[
            Query::variable_type("an int default against a float test", "a"),
            Query::variable_type("a float written at the call", "b"),
            Query::variable_type("a negative float default against an int test", "c"),
        ],
        expect![[r#"
            an int default against a float test
            - array<int, int>
            a float written at the call
            - array<int, float>
            a negative float default against an int test
            - float
        "#]],
    );
}
