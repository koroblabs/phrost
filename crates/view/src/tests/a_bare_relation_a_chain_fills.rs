//! A bare relation hint whose body narrows another relation the class declares.
//!
//! BookStack's `revisions(): HasMany { return $this->allRevisions()->where(...)->orderBy(...); }`:
//! the hint says only that it is some `HasMany`, the first call fills it -- `allRevisions()` is a
//! `HasMany<Revision, Page>` -- and each call after it returns `static`, which is that relation only
//! if its arguments travel with it from step to step.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

/** @template TModel */
class Builder
{
    /** @return TModel|null */
    public function first() {}
}

/**
 * @template TRelated
 * @template TDeclaring
 * @mixin Builder<TRelated>
 */
class HasMany
{
    /** @return $this */
    public function where(string $column) {}

    /** @return $this */
    public function orderBy(string $column) {}
}

abstract class Model
{
    /**
     * @template TRelated
     * @param class-string<TRelated> $related
     * @return HasMany<TRelated, $this>
     */
    public function hasMany(string $related) {}
}

//- /src/Page.php
<?php
namespace App;

use Orm\HasMany;

class Revision extends \Orm\Model {}

class Page extends \Orm\Model
{
    public function allRevisions(): HasMany
    {
        return $this->hasMany(Revision::class);
    }

    public function revisions(): HasMany
    {
        return $this->allRevisions()->where('type')->orderBy('id');
    }
}

function run(Page $page): void
{
    $a@all@ll = $page->allRevisions()->first();
    $r@narrowed@evision = $page->revisions()->first();
}
"#;

#[test]
fn a_relation_narrowed_from_another_keeps_its_model() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("the relation the call fills", "all"),
            Query::variable_type("narrowed by calls returning static", "narrowed"),
        ],
        expect![[r#"
            the relation the call fills
            - ?Revision
            narrowed by calls returning static
            - ?Revision
        "#]],
    );
}
