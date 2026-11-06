//! `@return Collection<TKey, TValue>` beside `@param Arrayable<TKey, TValue>|iterable<TKey, TValue>`.
//!
//! Laravel's `collect()`. The key slot is filled by the container's keys and the value slot by its
//! elements; a slot read only elements, so the key refused the whole contract and every
//! `collect($rows)` -- and every callback on what it made -- held nothing.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_collection_holds_what_the_container_it_was_made_from_held() {
    check_view(
        r#"
//- /src/helpers.php
<?php
namespace App;

class Route {}

/**
 * @template TKey
 * @template TValue
 */
interface Arrayable {}

/**
 * @template TKey of array-key
 * @template TValue
 */
class Collection
{
    /** @return TValue|null */
    public function first() {}
}

/**
 * @template TKey of array-key
 * @template TValue
 * @param Arrayable<TKey, TValue>|iterable<TKey, TValue>|null $value
 * @return Collection<TKey, TValue>
 */
function collect($value = []) {}

/**
 * @template TKey of array-key
 * @template TValue
 * @param iterable<TKey, TValue> $value
 * @return Collection<TKey, TValue>
 */
function plain($value) {}

/** @return Route[] */
function routes() {}

//- /src/run.php
<?php
namespace App;

function run(): void
{
    $coll = collect(routes());
    $c@coll@oll;
    $plainv = plain(routes());
    $p@plainv@lainv;
    $firstv = collect(routes())->first();
    $f@firstv@irstv;
}
"#,
        &[
            Query::variable_type("through a union of containers", "coll"),
            Query::variable_type("through one", "plainv"),
            Query::variable_type("and the chain goes on", "firstv"),
        ],
        expect![[r#"
            through a union of containers
            - Collection<int|string, Route>
            through one
            - Collection<int|string, Route>
            and the chain goes on
            - ?Route
        "#]],
    );
}
