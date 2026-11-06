//! A conditional return's `TRelated|null` branch, on a receiver written without arguments.
//!
//! The branch reader returned early when the receiver carried nothing, so neither an `@extends`
//! nor the bound was asked -- while a plain `@return TRelated|null` beside it fell to the bound.
//! And `?TRelated` passed as "something real", because only a bare name was tested. Laravel's
//! `$country->posts()->find(1)` read `?TRelatedModel`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_branch_naming_a_template_falls_to_its_bound() {
    check_view(
        r#"
//- /src/Relation.php
<?php
namespace App;

class Model {}

/** @template TRelated of Model */
abstract class Base
{
    /** @return ($id is array ? list<TRelated> : TRelated|null) */
    public function find($id) {}

    /** @return TRelated|null */
    public function first() {}
}

/**
 * @template TRelated of Model
 * @extends Base<TRelated>
 */
class Many extends Base {}

class Runner
{
    public function go(Many $many): void
    {
        $f@found@ound = $many->find(1);
        $p@plain@lain = $many->first();
    }
}
"#,
        &[
            Query::variable_type("the conditional's branch", "found"),
            Query::variable_type("a plain return beside it", "plain"),
        ],
        expect![[r#"
            the conditional's branch
            - ?Model
            a plain return beside it
            - ?Model
        "#]],
    );
}
