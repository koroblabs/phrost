//! `array{foo: int}` against `array{foo: int, ...}` -- whether a shape may hold keys it does not name.
//!
//! The two used to lower to one node, so nothing above could tell a shape that lists every key from
//! one that lists the keys worth mentioning. A shape is sealed unless it says `...`, which is how
//! PHPStan and Psalm read it. One that lost a key at lowering is open whatever it says: the key it
//! lost is one the array may hold, and sealed it would read as an extra key on every caller.
//!
//! No native return hints, for the reason `positional_shapes` gives: `check_hir` renders a
//! docblock's return only where the signature carries none.

use expect_test::expect;

use super::check_hir;

#[test]
fn a_shape_is_sealed_unless_it_says_it_is_not() {
    check_hir(
        r#"
//- /src/app.php
<?php

/** @return array{id: int} */
function sealed() {}

/** @return array{id: int, ...} */
function open() {}

/** @return array{id: int, ...<string, mixed>} */
function open_with_a_type() {}

/** @return array{} */
function empty_shape() {}

/** @return array{...} */
function nothing_named() {}
"#,
        expect![[r#"
            function sealed() doc: array{id: int} @ 4:10-4:16  [doc]
            function open() doc: array{id: int, ...} @ 7:10-7:14  [doc]
            function open_with_a_type() doc: array{id: int, ...} @ 10:10-10:26  [doc]
            function empty_shape() doc: array{} @ 13:10-13:21  [doc]
            function nothing_named() doc: array @ 16:10-16:23  [doc]
        "#]],
    );
}

/// A key a constant names cannot be lowered, so the shape that wrote it says less than it did.
#[test]
fn a_shape_that_lost_a_key_is_open() {
    check_hir(
        r#"
//- /src/app.php
<?php

/** @return array{id: int, Thing::KEY: string} */
function lost() {}
"#,
        expect![[r#"
            function lost() doc: array{id: int, ...} @ 4:10-4:14  [doc]
        "#]],
    );
}

/// `list{...}` is a shape too, keyed `0` upwards, and says so: the same keys spelled `array{...}`
/// promise nothing about their order.
#[test]
fn a_list_shape_is_told_from_the_array_with_its_keys() {
    check_hir(
        r#"
//- /src/app.php
<?php

/** @return list{string, int} */
function listed() {}

/** @return array{string, int} */
function keyed() {}
"#,
        expect![[r#"
            function listed() doc: list{0: string, 1: int} @ 4:10-4:16  [doc]
            function keyed() doc: array{0: string, 1: int} @ 7:10-7:15  [doc]
        "#]],
    );
}
