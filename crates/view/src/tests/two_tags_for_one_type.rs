//! A declaration that writes both `@return` and `@phpstan-return`, and which one is the statement.
//!
//! Doctrine's `EntityRepository::find` writes exactly this:
//!
//! ```text
//! @return object|null The entity instance or NULL if the entity can not be found.
//! @phpstan-return ?T
//! ```
//!
//! The plain tag is there for a reader that cannot hold the precise one -- PHP has no syntax for `T`,
//! and `object|null` is the closest thing that is *true* -- so the prefixed tag is not a second
//! opinion, it is the statement. Taking whichever came first made `find()` an `object|null` while
//! `findOneBy()` four lines below, which writes only the prefixed tag, answered the entity: the same
//! class, the same shape, one paragraph of prose apart.
//!
//! Every tag kind and not only `@return`, because every one has the twin: on one real vendor tree
//! `@phpstan-param` is 588 occurrences against `@phpstan-return`'s 381, `@psalm-return` 96, and 524 of
//! the whole lot are in `doctrine/`.
//!
//! # What it costs
//!
//! Drupal core writes the same shape on `ListInterface::get()`, with a *resolvable* plain tag --
//! `@return TypedDataInterface|null` beside `@phpstan-return ?T` -- and most code holds a plain
//! `FieldItemListInterface` with nothing to bind `T` to. Six member accesses on the reference project
//! stopped resolving, and two `undefined-method` findings went with them: the chain used to answer
//! `TypedDataInterface`, whose declared surface is almost empty, so the *next* call on it was reported
//! undefined on code that runs. Six confidently wrong answers became six honest silences.

use expect_test::expect;

use super::{Query, check_view};

const PROJECT: &str = r#"
//- /src/Repo.php
<?php
namespace App;

class Row
{
    public function label(): string {}
}

/** @template T of Row */
class Repo
{
    /**
     * @return object|null The row, or NULL if there is none.
     * @phpstan-return ?T
     */
    public function plainFirst() {}

    /**
     * @phpstan-return ?T
     * @return object|null
     */
    public function preciseFirst() {}

    /**
     * @psalm-return ?T
     */
    public function psalmOnly() {}

    /**
     * @return object|null
     */
    public function plainOnly() {}
}

class Holder
{
    /** @var Repo<Row> */
    public Repo $rows;
}

class Reader
{
    public function run(Holder $holder): void
    {
        $plainFirst = $holder->rows->plainFirst();
        $preciseFirst = $holder->rows->preciseFirst();
        $psalmOnly = $holder->rows->psalmOnly();
        $plainOnly = $holder->rows->plainOnly();

        $plainF@plainFirst@irst;
        $preciseF@preciseFirst@irst;
        $psalmO@psalmOnly@nly;
        $plainO@plainOnly@nly;
    }
}
"#;

#[test]
fn the_prefixed_tag_is_the_statement_whichever_order_they_are_written_in() {
    check_view(
        PROJECT,
        &[
            Query::variable_type("the plain tag written first", "plainFirst"),
            Query::variable_type("and written second", "preciseFirst"),
            Query::variable_type("psalm's spelling of the same", "psalmOnly"),
            Query::variable_type("no prefixed tag, so the plain one stands", "plainOnly"),
        ],
        expect![[r#"
            the plain tag written first
            - ?Row
            and written second
            - ?Row
            psalm's spelling of the same
            - ?Row
            no prefixed tag, so the plain one stands
            - ?object
        "#]],
    );
}

/// The plain tag is for a reader that cannot hold the precise one, and a prefixed tag that lowers to
/// nothing at all -- `key-of<T>` beside no template, which nothing here models -- is exactly that
/// reader's case. One that names the declaration's *own* template does lower, and does not hand
/// over: `wp_unslash()` writes `@return string|array` above a prefixed tag that gives back whatever
/// it was handed, and the plain claim taken for a `$_POST` value was 314 findings on WordPress about
/// arrays nobody passes. Being a conditional, it is read with the template standing for anything,
/// which is what a call nothing settles gets back.
#[test]
fn a_prefixed_tag_nothing_can_hold_hands_over_and_a_refused_one_does_not() {
    check_view(
        r#"
//- /src/functions.php
<?php
/**
 * @return array<string, int>
 * @phpstan-return key-of<Settings>
 */
function unmodelled() {}

/**
 * @return string|array
 *
 * @phpstan-template T
 * @phpstan-param T $value
 * @phpstan-return (T is string ? string : T)
 */
function unslash( $value ) {}

function run( $value ) {
    $a = unmodelled();
    $@a@a;
    $b = unslash( $value );
    $@b@b;
}
"#,
        &[
            Query::variable_type("a prefixed tag that lowers to nothing", "a"),
            Query::variable_type("a prefixed tag naming its own template", "b"),
        ],
        expect![[r#"
            a prefixed tag that lowers to nothing
            - array<string, int>
            a prefixed tag naming its own template
            - mixed
        "#]],
    );
}
