//! What `::` reaches on a class that answers `__callStatic`.
//!
//! PHP sends a static call to a name the class does not declare to `__callStatic`, which is how
//! `User::where()` reaches Eloquent's query builder -- the most written line in a Laravel project,
//! and `User::wher` offered nothing while `$user->wher` offered the builder's methods. An instance
//! method the class does declare is still a fatal error through `::`: PHP looks for it first and
//! finds it non-static, so `__callStatic` never runs.

use expect_test::expect;

use crate::tests::{Query, check};

const BUILDER: &str = r#"
//- /src/Builder.php
<?php
namespace App;

class Builder
{
    public array $wheres = [];

    public function where(string $column): static {}
    public function whereIn(string $column, array $values): static {}
}
"#;

fn check_with_builder(client: &str, queries: &[Query], expect: expect_test::Expect) {
    check(&format!("{BUILDER}\n{client}"), queries, expect);
}

#[test]
fn a_mixin_method_is_reached_through_call_static() {
    check_with_builder(
        r#"
//- /src/User.php
<?php
namespace App;

/** @mixin Builder */
class User
{
    public function save(): bool {}
    public static function find(int $id): ?static {}
    public static function __callStatic(string $method, array $arguments): mixed {}
}

function run(): void
{
    User::@here@
}
"#,
        &[Query::completion("static access", "here")],
        expect![[r#"
            static access
            - __callStatic [method]  : mixed  shown last  writes `(|);`
            - find [method]  : ?static  writes `(|);`
            - where [method]  : static  writes `(|);`
            - whereIn [method]  : static  writes `(|);`
            - class [const]  App\User
            - accepting the first leaves `User::find`
        "#]],
    );
}

#[test]
fn without_call_static_a_mixin_method_is_not_static() {
    check_with_builder(
        r#"
//- /src/User.php
<?php
namespace App;

/** @mixin Builder */
class User
{
    public static function find(int $id): ?static {}
}

function run(): void
{
    User::wh@here@
}
"#,
        &[Query::completion("no __callStatic", "here")],
        expect![[r#"
            no __callStatic
            - (nothing offered)
        "#]],
    );
}
