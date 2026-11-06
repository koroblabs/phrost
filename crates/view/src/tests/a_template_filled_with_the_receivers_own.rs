//! An ancestor's template filled with a type written in the receiver's own templates.
//!
//! Laravel's relations are the case: `Relation::getResults()` returns `TResult`, and `HasMany` fills
//! that with `Collection<int, TRelatedModel>` -- its *own* `TRelatedModel`. So on a
//! `HasMany<Comment>` the first substitution lands on `Collection<int, TRelatedModel>`, and the
//! receiver's `Comment` has to go into it second. Stopping after the first answered a container of
//! a class nothing declares.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /src/Relations.php
<?php
namespace App;

class Comment {}

/** @template TValue */
class Collection {}

/**
 * @template TRelated
 * @template TResult
 */
abstract class Relation
{
    /** @return TResult */
    public function getResults() {}
}

/**
 * @template TRelated
 * @extends Relation<TRelated, Collection<TRelated>>
 */
class HasMany extends Relation {}

class Post
{
    /** @return HasMany<Comment> */
    public function comments(): HasMany {}
}

function run(Post $post): void
{
    $a@all@ll = $post->comments()->getResults();
}
"#;

#[test]
fn the_receivers_argument_goes_into_what_the_ancestor_was_filled_with() {
    check_view(
        FIXTURE,
        &[Query::variable_type(
            "getResults() on HasMany<Comment>",
            "all",
        )],
        expect![[r#"
            getResults() on HasMany<Comment>
            - Collection<Comment>
        "#]],
    );
}
