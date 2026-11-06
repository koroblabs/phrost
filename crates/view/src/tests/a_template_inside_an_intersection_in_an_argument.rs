//! `Collection<int, TRelatedModel&object{pivot: TPivotModel}>` -- a template inside an intersection
//! that is itself a generic argument.
//!
//! How `BelongsToMany` states what it loads, once in `get()` and once in the `@extends` that
//! `getResults()` reads through `TResult`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_template_inside_an_intersection_inside_an_argument_is_substituted() {
    check_view(
        r#"
//- /src/Types.php
<?php
namespace App;

class User {}
class Pivot {}

/**
 * @template TKey
 * @template TValue
 * @implements \IteratorAggregate<TKey, TValue>
 */
class Coll implements \IteratorAggregate
{
    /** @return TValue */
    public function one() {}
}

/**
 * @template TRelated
 * @template TResult
 */
abstract class Relation
{
    /** @return TResult */
    abstract public function getResults();
}

/**
 * @template TRelated
 * @template TPivot
 * @extends Relation<TRelated, Coll<int, TRelated&object{pivot: TPivot}>>
 */
class Related extends Relation
{
    /** @return Coll<int, TRelated&object{pivot: TPivot}> */
    public function get() {}

    /** @inheritDoc */
    public function getResults() {}
}

//- /src/run.php
<?php
namespace App;

/** @param Related<User, Pivot> $users */
function run(Related $users): void
{
    $all = $users->get();
    $a@all@ll;
    $one = $all->one();
    $o@one@ne;
    $results = $users->getResults();
    $r@results@esults;
    foreach ($results as $user) {
        $u@user@ser;
        $pivot = $user->pivot;
        $p@pivot@ivot;
    }
}
"#,
        &[
            Query::variable_type("written on the method", "all"),
            Query::variable_type("an element of it", "one"),
            Query::variable_type("through the parent's template", "results"),
            Query::variable_type("iterated", "user"),
            Query::variable_type("the property the shape names", "pivot"),
        ],
        expect![[r#"
            written on the method
            - Coll<int, User&object{pivot: Pivot}>
            an element of it
            - User&object{pivot: Pivot}
            through the parent's template
            - Coll<int, User&object{pivot: Pivot}>
            iterated
            - User&object{pivot: Pivot}
            the property the shape names
            - Pivot
        "#]],
    );
}
