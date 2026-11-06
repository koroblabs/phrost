//! A method whose return is the class one fixed catalog key names: Laravel's signed-in user.
//!
//! `$request->user()` declares `mixed` and a guard's `user()` an `Authenticatable`; which model
//! comes back is `config/auth.php`'s to say, and the provider files it only where every provider
//! names the same one. Nullable, because nobody may be signed in. A project that filed no key keeps
//! the declaration.

use expect_test::expect;

use phrost_index::KeyedReturnRule;

use super::{Query, check_view_with};

const FIXTURE: &str = r#"
//- /src/app.php
<?php
namespace App;

interface Authenticatable {}

class User implements Authenticatable {}

class Request
{
    /** @return mixed */
    public function user($guard = null) {}
}

class Guard
{
    public function user(): ?Authenticatable {}
}

function run(Request $request, Guard $guard): void
{
    $r@request@equest = $request->user();
    $g@guard@uard = $guard->user();
}
"#;

const QUERIES: &[Query] = &[
    Query::variable_type("the request's user", "request"),
    Query::variable_type("a guard's user", "guard"),
];

fn bind(world: &mut phrost_harness::World) {
    for class in ["App\\Request", "App\\Guard"] {
        world.contracts_mut().bind_keyed_return(
            "user",
            KeyedReturnRule {
                class: class.into(),
                catalog: "auth".into(),
                key: "user".into(),
                nullable: true,
            },
        );
    }
}

#[test]
fn the_configured_model_is_what_comes_back() {
    check_view_with(
        FIXTURE,
        QUERIES,
        |world| {
            bind(world);
            world
                .contracts_mut()
                .catalog_entry("auth", "user", Some("App\\User"));
        },
        expect![[r#"
            the request's user
            - ?User
            a guard's user
            - ?User
        "#]],
    );
}

#[test]
fn a_project_that_named_no_model_keeps_the_declaration() {
    check_view_with(
        FIXTURE,
        QUERIES,
        bind,
        expect![[r#"
            the request's user
            - mixed
            a guard's user
            - ?Authenticatable
        "#]],
    );
}
