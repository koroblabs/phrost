//! A generic return a framework release declared bare, filled from the receiver.
//!
//! Laravel 11 writes `@return \Illuminate\Pagination\LengthAwarePaginator` over
//! `Eloquent\Builder::paginate()`, where later releases write `LengthAwarePaginator<int, TModel>`.
//! The framework states the arguments (see the seam's `FilledReturn`) and the receiver supplies
//! `TModel` -- directly on a builder, and through a relation's `@mixin` on a relation. The body is
//! not asked first: `simplePaginate()` hands back the concrete page of nothing in particular.

use expect_test::expect;

use super::{Query, check_view_ingested_with};

fn pages_are_of_the_builders_model(world: &mut phrost_harness::World) {
    for method in ["paginate", "simplePaginate"] {
        world.contracts_mut().bind_filled_return(
            method,
            phrost_index::FilledReturnRule {
                class: "Orm\\Builder".into(),
                arguments: Box::new([
                    phrost_index::FilledArgumentRule::Builtin(phrost_model::BuiltinTy::Int),
                    phrost_index::FilledArgumentRule::Template("TModel".into()),
                ]),
            },
        );
    }
}

#[test]
fn a_page_holds_the_model_of_the_builder_that_made_it() {
    check_view_ingested_with(
        r#"
//- /vendor/Orm.php
<?php
namespace Orm;

/**
 * @template TKey
 * @template TValue
 */
interface Paginator {}

/**
 * @template TKey
 * @template TValue
 * @implements Paginator<TKey, TValue>
 */
class SimplePage implements Paginator {}

/**
 * @template TKey
 * @template TValue
 */
class Page {}

/** @template TModel */
class Builder
{
    /** @return Page */
    public function paginate(int $count) { return new Page(); }

    /** @return Paginator */
    public function simplePaginate(int $count) { return $this->page(); }

    /** @return SimplePage */
    protected function page() { return new SimplePage(); }
}

/**
 * @template TRelated
 * @mixin Builder<TRelated>
 */
class HasMany
{
    public function __call(string $method, array $arguments): mixed {}
}

//- /src/run.php
<?php
namespace App;

class Post {}

/**
 * @param \Orm\Builder<Post> $posts
 * @param \Orm\HasMany<Post> $relation
 */
function run($posts, $relation): void
{
    $page = $posts->paginate(10);
    $p@page@age;
    $simple = $posts->simplePaginate(10);
    $s@simple@imple;
    $related = $relation->paginate(10);
    $r@related@elated;
}
"#,
        &[
            Query::variable_type("a builder's page", "page"),
            Query::variable_type(
                "a contract it declares, not what its body returns",
                "simple",
            ),
            Query::variable_type("a relation's, through its mixin", "related"),
        ],
        pages_are_of_the_builders_model,
        expect![[r#"
            a builder's page
            - Page<int, Post>
            a contract it declares, not what its body returns
            - Paginator<int, Post>
            a relation's, through its mixin
            - Page<int, Post>
        "#]],
    );
}
