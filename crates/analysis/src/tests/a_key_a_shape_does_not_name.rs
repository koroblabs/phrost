//! A key a shape does not name is not judged, `...` or not; a key it requires has to be there.
//!
//! This read shapes as sealed for a while -- PHPStan's bleeding edge, Psalm and Mago do -- and on
//! the reference project that was 36 errors where a helper's `@param array{city?: mixed, ...}`
//! names the keys it reads and its callers pass the whole record. PHPStan at level 8 with that
//! project's configuration reports none of them, and none was a defect, so a key the value has and
//! the shape does not name is the callee's to ignore. What is still judged is the half PHPStan
//! judges: a required key the value lacks -- including one an `unset` took away after the
//! declaration typed it -- and a key's type.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_literal_with_a_key_the_shape_does_not_name() {
    check(
        r#"
//- /app.php
<?php
/** @param array{foo: int} $shape */
function sealed(array $shape): void {}

/** @param array{foo: int, ...} $shape */
function open(array $shape): void {}

function run(): void
{
    sealed(['foo' => 1, 'buz' => 2.0]);
    open(['foo' => 1, 'buz' => 2.0]);
}
"#,
        expect![""],
    );
}

#[test]
fn a_declared_shape_wider_than_the_one_it_is_handed_to() {
    check(
        r#"
//- /app.php
<?php
/** @param array{foo: int} $shape */
function sealed(array $shape): void {}

/** @param array{foo: int, buz: float} $wider */
function run(array $wider): void
{
    sealed($wider);
}
"#,
        expect![""],
    );
}

#[test]
fn an_empty_literal_where_a_key_is_required() {
    check(
        r#"
//- /app.php
<?php
/** @param array{foo: int} $shape */
function sealed(array $shape): void {}

/** @param array{foo?: int} $shape */
function optional(array $shape): void {}

function run(): void
{
    sealed([]);
    optional([]);
}
"#,
        expect![[r#"
            10:12-10:14 error [incompatible-argument] $shape takes array{foo: int}, and this is array
        "#]],
    );
}

/// A parameter is the record its shape describes, and a write into it is folded in the way a write
/// into a literal is.
#[test]
fn a_key_written_into_a_declared_shape_is_a_key_it_has() {
    check(
        r#"
//- /app.php
<?php
/** @param array{foo: int, bar: int} $shape */
function both(array $shape): void {}

/** @param array{foo: int} $shape */
function sealed(array $shape): void {}

/** @param array{foo: int} $row */
function run(array $row): void
{
    $row['bar'] = 2;
    both($row);
    sealed($row);
}
"#,
        expect![""],
    );
}

/// And a key an `unset` took out is one it no longer has, whether the array was a literal or the
/// shape a call returned: the shape requires `foo`, and neither still holds it. The last call is the
/// control: the same shape with nothing unset.
#[test]
fn a_key_unset_from_a_record_or_a_shape_is_gone() {
    check(
        r#"
//- /app.php
<?php
/** @param array{foo: int} $shape */
function takes(array $shape): void {}

/** @return array{foo: int, tmp: int} */
function make(): array { return ['foo' => 1, 'tmp' => 2]; }

function run(): void
{
    $literal = ['foo' => 1, 'tmp' => 2];
    unset($literal['foo']);
    takes($literal);

    $made = make();
    unset($made['foo']);
    takes($made);

    takes(make());
}
"#,
        expect![[r#"
            12:11-12:19 error [incompatible-argument] $shape takes array{foo: int}, and this is array{tmp: int}
            16:11-16:16 error [incompatible-argument] $shape takes array{foo: int}, and this is array{tmp: int}
        "#]],
    );
}

/// A list is keyed `0` upwards in the order written, so the same two keys reversed are not one.
#[test]
fn a_literal_keyed_out_of_order_is_not_a_list() {
    check(
        r#"
//- /app.php
<?php
/** @param list{string, string} $pair */
function pair(array $pair): void {}

function run(): void
{
    pair([0 => 'x', 1 => 'y']);
    pair([1 => 'x', 0 => 'y']);
}
"#,
        expect![[r#"
            8:10-8:30 error [incompatible-argument] $pair takes list{0: string, 1: string}, and this is array{1: non-falsy-string, 0: non-falsy-string}
        "#]],
    );
}
