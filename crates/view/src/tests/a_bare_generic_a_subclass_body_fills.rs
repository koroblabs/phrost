//! A bare generic declaration whose body returns a subclass that fills its templates.
//!
//! Laravel's old factory stubs: `@return \Illuminate\Database\Eloquent\Factories\Factory` over a state
//! method whose body is `return $this->state(...)` -- a `static`, so on a `UserFactory` the
//! `UserFactory` that `@extends Factory<User>`, and what `create()` makes is a `User`. The bare name
//! says only that it is some factory. A declaration naming a class with no templates is a whole
//! claim, and a subclass the body returns does not replace it.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

/** @template TModel */
abstract class Factory
{
    /** @return static */
    public function state(array $state) {}

    /** @return TModel */
    public function create() {}
}

class Animal {}

//- /src/Factories.php
<?php
namespace App;

class User {}
class Dog extends \Orm\Animal {}

/** @extends \Orm\Factory<User> */
class UserFactory extends \Orm\Factory
{
    /** @return \Orm\Factory */
    public function unverified()
    {
        return $this->state([]);
    }

    /** @return \Orm\Animal */
    public function pet()
    {
        return new Dog();
    }
}

function run(UserFactory $users): void
{
    $u@unverified@nverified = $users->unverified()->create();
    $p@pet@et = $users->pet();
}
"#;

#[test]
fn a_bare_generic_is_filled_by_the_subclass_its_body_returns() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type(
                "a state method of a factory, through the subclass",
                "unverified",
            ),
            Query::variable_type("a declaration with nothing to fill stands", "pet"),
        ],
        expect![[r#"
            a state method of a factory, through the subclass
            - User
            a declaration with nothing to fill stands
            - Animal
        "#]],
    );
}
