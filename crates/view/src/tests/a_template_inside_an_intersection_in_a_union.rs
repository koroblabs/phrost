//! `@return (TRelatedModel&object{pivot: TPivotModel})|null` -- a template one level inside a union.
//!
//! How `BelongsToMany::first()` is written, so `$shelf->books()->first()` typed nothing: the
//! substitution took a union apart operand by operand, and an operand that was itself an
//! intersection was kept whole, with a template name in it no class answers to. Each nested union
//! and intersection is now the same question again, and the `object` beside a class says nothing
//! the class does not.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_template_inside_an_intersection_inside_a_union_is_substituted() {
    check_view(
        r#"
//- /src/Types.php
<?php
namespace App;

class Book {}
class Pivot {}

/**
 * @template TRelated
 * @template TPivot
 */
class Related
{
    /** @return (TRelated&object{pivot: TPivot})|null */
    public function first() { return null; }
}

//- /src/run.php
<?php
namespace App;

/** @param Related<Book, Pivot> $books */
function run(Related $books): void
{
    $first = $books->first();
    $f@first@irst;
}
"#,
        &[Query::variable_type("the relation's first", "first")],
        expect![[r#"
            the relation's first
            - (Book&object{pivot: Pivot})|null
        "#]],
    );
}
