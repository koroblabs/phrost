//! A chain of tests whose argument no link can judge whole, judged one part at a time.
//!
//! Eloquent's `find()`: `($key is Model ? TModel|null : ($key is (Arrayable|array) ? static :
//! TModel|null))`. A `?User` is a `User` or a null; the first test holds for one and fails for the
//! other, so it settled nothing and the call fell back to the union of every branch -- a collection
//! included, because a `User` is also `Arrayable`. The second test only ever sees what the first
//! left, which is the null. PHPStan 2.2.15 had the same bug (phpstan-src#6586).

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_second_link_sees_only_what_the_first_left() {
    check_view(
        r#"
//- /src/Collection.php
<?php
namespace App;

interface Arrayable {}

class Model implements Arrayable {}

final class User extends Model {}

/** @template TModel of Model */
final class Collection
{
    /**
     * @return ($key is Model ? TModel|null : ($key is (Arrayable|array<mixed>) ? static : TModel|null))
     */
    public function find(mixed $key): mixed {}
}

class Caller
{
    /** @param Collection<User> $users */
    public function run(Collection $users, ?User $user, mixed $anything): void
    {
        $found = $users->find($user);
        $f@found@ound;
        $many = $users->find([1, 2]);
        $m@many@any;
        $unknown = $users->find($anything);
        $u@unknown@nknown;
    }
}
"#,
        &[
            Query::variable_type("a model or nothing", "found"),
            Query::variable_type("a list of keys", "many"),
            Query::variable_type("a part nothing settles leaves the declaration", "unknown"),
        ],
        expect![[r#"
            a model or nothing
            - ?User
            a list of keys
            - Collection<User>
            a part nothing settles leaves the declaration
            - User|Collection|null
        "#]],
    );
}
