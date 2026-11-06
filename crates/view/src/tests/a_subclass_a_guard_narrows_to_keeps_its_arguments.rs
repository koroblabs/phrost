//! A value narrowed to a subclass keeps the type arguments it already had.
//!
//! `Enumerable<int, User>` narrowed by `instanceof Collection` is a `Collection<int, User>`, because
//! `Collection` passes its own templates up to `Enumerable` in the same places. Narrowed to the bare
//! class, the guard took away what the declaration said. An asserted subclass is the same claim
//! reached through a tag, and follows the same rule.

use expect_test::expect;

use super::{Query, check_view};

const HIERARCHY: &str = r#"
//- /src/Types.php
<?php
namespace App;

class User {}

/**
 * @template TKey
 * @template TValue
 */
interface Enumerable {}

/**
 * @template TKey
 * @template TValue
 * @implements Enumerable<TKey, TValue>
 */
class Collection implements Enumerable {}

/**
 * @template TValue
 * @extends Collection<int, TValue>
 */
class Values extends Collection {}

/** @phpstan-assert Values $actual */
function assertValues(mixed $actual): void {}
"#;

#[test]
fn a_guard_carries_the_arguments_to_the_class_it_names() {
    check_view(
        &[
            HIERARCHY,
            r#"
//- /src/run.php
<?php
namespace App;

/** @param Enumerable<int, User> $items */
function run(Enumerable $items): void
{
    if ($items instanceof Collection) {
        $it@collection@ems;
    }
    if ($items instanceof Values) {
        $it@values@ems;
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("an implementation passing both templates up", "collection"),
            Query::variable_type("a subclass passing one of them", "values"),
        ],
        expect![[r#"
            an implementation passing both templates up
            - Collection<int, User>
            a subclass passing one of them
            - Values<User>
        "#]],
    );
}

#[test]
fn an_asserted_subclass_carries_them_too() {
    check_view(
        &[
            HIERARCHY,
            r#"
//- /src/run.php
<?php
namespace App;

/** @param Collection<int, User> $items */
function run(Collection $items): void
{
    assertValues($items);
    $it@after@ems;
}
"#,
        ]
        .concat(),
        &[Query::variable_type("after the assertion", "after")],
        expect![[r#"
            after the assertion
            - Values<User>
        "#]],
    );
}
