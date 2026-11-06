//! `/** @var T $x */ $x = f($x);` -- the annotation is about what the assignment produces.
//!
//! Inside that assignment's own right-hand side the name still holds what it held before. Read as
//! the annotated type there, `/** @var UserInterface $user */ $user = $storage->load($user);`
//! passed a `UserInterface` where the code passes the parameter's `?int`, which is the null PHPStan
//! reports on the reference project; and Symfony's `/** @var SubscribedService $attribute */
//! $attribute = $attribute->newInstance();` called `newInstance()` on the attribute class rather
//! than on the reflection it is.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_right_hand_side_reads_the_value_before_the_annotation() {
    check_view(
        r#"
//- /src/Users.php
<?php
namespace App;

class User {}

class Storage
{
    public function load(int|string $id): ?object {}
}

class Users
{
    public function run(Storage $storage, ?int $user): void
    {
        /** @var User $user */
        $user = $storage->load($u@inside@ser);
        $u@after@ser;
    }
}
"#,
        &[
            Query::variable_type("inside its own assignment", "inside"),
            Query::variable_type("after it", "after"),
        ],
        expect![[r#"
            inside its own assignment
            - ?int
            after it
            - User
        "#]],
    );
}
