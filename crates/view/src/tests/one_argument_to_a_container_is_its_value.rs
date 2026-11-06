//! `Collection<Page>` written on a `<TKey, TValue>` collection: the one argument is the value.
//!
//! The convention a `foreach` was already read by, and a member's return has to read it the same
//! way, or `foreach ($pages as $page)` and `$pages->first()` disagree about one value. Only for a
//! container -- a class PHP iterates for what it yields. Elsewhere one argument is the first
//! template, as written.

use expect_test::expect;

use super::{Query, check_view};

const CLASSES: &str = r#"
//- /src/Types.php
<?php
namespace App;

class Page {}

/**
 * @template TKey
 * @template TValue
 * @implements \IteratorAggregate<TKey, TValue>
 */
class Collection implements \IteratorAggregate
{
    /** @return TValue|null */
    public function first() { return null; }
    public function getIterator(): \Iterator { return new \ArrayIterator([]); }
}

/**
 * @template TOk
 * @template TError
 */
class Result
{
    /** @return TOk */
    public function ok() { return null; }
}
"#;

#[test]
fn a_containers_one_argument_is_what_its_members_return() {
    check_view(
        &[
            CLASSES,
            r#"
//- /src/run.php
<?php
namespace App;

/**
 * @param Collection<Page> $pages
 * @param Result<Page> $result
 */
function run(Collection $pages, Result $result): void
{
    $first = $pages->first();
    $f@first@irst;
    $ok = $result->ok();
    $o@ok@k;
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("a container's member", "first"),
            Query::variable_type("anything else's, by position", "ok"),
        ],
        expect![[r#"
            a container's member
            - ?Page
            anything else's, by position
            - Page
        "#]],
    );
}
