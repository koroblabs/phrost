//! `@return Coll<int, TModel>` on a receiver that supplied nothing for `TModel`.
//!
//! The container half of `a_template_nothing_supplied`: a bare `@return TModel` already fell to the
//! bound, and the same template one level in answered nothing at all -- the substitution into a
//! container's slots only ran when the *value* carried arguments, so an `@extends` between the two
//! and the template's bound were never asked. Beside its element in a union it was worse: the slot
//! kept the name, and `$users[0]->name` asked for a member of a class called `TModel`. Laravel's
//! `Factory::create()` is `Collection<int, TModel>|TModel`, and a factory declares no `@extends`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_container_of_a_template_nothing_supplied_holds_its_bound() {
    check_view(
        r#"
//- /src/Factory.php
<?php
namespace App;

class Model {}
class User extends Model {}

/**
 * @template TKey
 * @template TValue
 */
class Coll {}

/** @template TModel of Model */
abstract class Factory
{
    /** @return Coll<int, TModel> */
    public function many() {}

    /** @return Coll<int, TModel>|TModel */
    public function create() {}
}

class AnyFactory extends Factory {}

/** @extends Factory<User> */
class UserFactory extends Factory {}

class Runner
{
    public function go(AnyFactory $any, UserFactory $users): void
    {
        $m@many@any = $any->many();
        $c@create@reate = $any->create();
        $u@extended@sers = $users->many();
    }
}
"#,
        &[
            Query::variable_type("the bound, in the slot", "many"),
            Query::variable_type("beside its element in a union", "create"),
            Query::variable_type("what `@extends` filled in", "extended"),
        ],
        expect![[r#"
            the bound, in the slot
            - Coll<int, Model>
            beside its element in a union
            - Coll<int, Model>|Model
            what `@extends` filled in
            - Coll<int, User>
        "#]],
    );
}
