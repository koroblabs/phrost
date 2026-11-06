//! A model's columns written as `@property` tags, and its relations' counts, offered in a list.
//!
//! A tag no method backs is a column, so the query builder's `__call` answers a `where` for it; a
//! tag a method backs is a relation, and `whereChapter` would ask for a column that is not there.
//! And each relation has the count a query's `withCount()` puts beside it.

use expect_test::expect;
use phrost_harness::{StatedPropertiesRule, World};

use super::{Query, check_with};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

class Query
{
    public function __call(string $name, array $arguments): mixed {}
}

/**
 * @template TModel
 * @mixin Query
 */
class Builder
{
    public function __call(string $name, array $arguments): mixed {}
}

abstract class Model
{
    public static function __callStatic(string $name, array $arguments): mixed {}
    public function __call(string $name, array $arguments): mixed {}
    public function __get(string $key): mixed {}
}

/** @template TRelated */
abstract class Relation
{
    /** @return TRelated */
    public function getResults() {}
}

/**
 * @template TRelated
 * @extends Relation<TRelated>
 */
class HasMany extends Relation {}

//- /src/Page.php
<?php
namespace App;

/**
 * @property string $html
 * @property Chapter $chapter
 */
class Page extends \Orm\Model
{
    /** @return \Orm\HasMany<Chapter> */
    public function chapter(): \Orm\HasMany {}

    /** @return \Orm\HasMany<Page> */
    public function revisions(): \Orm\HasMany {}

    public function attachments(): \Orm\HasMany {}
}

class Chapter extends \Orm\Model {}

//- /src/Use.php
<?php
namespace App;

function wheres(): void
{
    Page::where@where@
}

function counts(Page $page): void
{
    $page->rev@count@
}

function untyped(Page $page): void
{
    $page->att@untyped@
}
"#;

fn rules(world: &mut World) {
    world.add_implied_mixin("Orm\\Model", "Orm\\Builder", true);
    world
        .contracts_mut()
        .bind_fluent_prefix("Orm\\Query", "where");
    world.contracts_mut().bind_method_property(
        "Orm\\Model",
        "Orm\\Relation",
        "getResults",
        Some("_count"),
        &[],
    );
    world
        .contracts_mut()
        .bind_stated_properties(StatedPropertiesRule {
            class: "Orm\\Model".into(),
            typed: Vec::new(),
            listed: Vec::new(),
            listed_by_attribute: Vec::new(),
            spellings: Vec::new(),
            wrapping: Vec::new(),
            classes: Vec::new(),
            custom: None,
            implicit: Vec::new(),
            key: None,
            nullable: true,
        });
}

#[test]
fn a_tagged_column_has_a_where_and_a_relation_has_a_count() {
    check_with(
        FIXTURE,
        &[
            Query::completion("a where for the column, none for the relation", "where"),
            Query::completion("a relation and its count", "count"),
            Query::completion(
                "a relation saying nothing of what it loads still has a count",
                "untyped",
            ),
        ],
        rules,
        expect![[r#"
            a where for the column, none for the relation
            - whereHtml [method]  Page::$html  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `Page::whereHtml`
            a relation and its count
            - revisions [method]  : HasMany<Page>  writes `();`
            - revisions [property]  Page
            - revisions_count [property]  ?int
            - ... and the client must ask again
            - accepting the first leaves `$page->revisions`
            a relation saying nothing of what it loads still has a count
            - __callStatic [method]  : mixed  shown last  writes `(|);`
            - attachments [method]  : HasMany  writes `();`
            - attachments_count [property]  ?int
            - chapter_count [property]  ?int
            - ... and the client must ask again
            - accepting the first leaves `$page->attachments`
        "#]],
    );
}
