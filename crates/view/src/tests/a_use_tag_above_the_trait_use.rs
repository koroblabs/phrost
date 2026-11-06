//! `/** @use Trait<T> */` written above the trait use, inside the class body.
//!
//! Where PHPStan's documentation and Laravel both put it, and where lowering did not look: only the
//! class's own docblock was read, so `Eloquent\Builder`'s `use BuildsQueries` supplied the trait
//! nothing and every `$query->first()` answered a template called `TValue`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_trait_template_is_bound_by_the_tag_above_its_use() {
    check_view(
        r#"
//- /vendor/orm/Builds.php
<?php
namespace Orm;

/** @template TValue */
trait BuildsQueries
{
    /** @return TValue|null */
    public function first() {}
}

//- /vendor/orm/Builder.php
<?php
namespace Orm;

/** @template TModel */
class Builder
{
    /** @use BuildsQueries<TModel> */
    use BuildsQueries;

    /** @return $this */
    public function where() { return $this; }

    /** @return TModel */
    public function one() {}
}

//- /src/User.php
<?php
namespace App;

class User
{
    /** @param \Orm\Builder<User> $b */
    public function run($b): void
    {
        $x = $b->first();
        $@x@x;
        $y = $b->where()->first();
        $@y@y;
        $z = $b->one();
        $@z@z;
    }
}
"#,
        &[
            Query::variable_type(
                "the trait's method on a receiver that supplied the class's template",
                "x",
            ),
            Query::variable_type("and after a fluent call", "y"),
            Query::variable_type("the class's own method, for comparison", "z"),
        ],
        expect![[r#"
            the trait's method on a receiver that supplied the class's template
            - ?User
            and after a fluent call
            - ?User
            the class's own method, for comparison
            - User
        "#]],
    );
}
