//! What the cursor is on inside a string that a call reads as a key.
//!
//! The only place a position query looks inside a literal, and the reason is that the literal is the
//! only thing there that names anything: `\Drupal::service('application.manager')` is declared to
//! return `object`, so the id carries the whole answer.

use expect_test::expect;

use super::{Query, check_view_with};

/// The fixture every test here shares: a container, two classes, and a call reading an id.
const PROJECT: &str = r#"
//- /src/Container.php
<?php
namespace Dep;

class Container
{
    public function get(string $id): mixed
    {
        return NULL;
    }

    public function has(string $id): bool
    {
        return TRUE;
    }
}

//- /src/Manager.php
<?php
namespace App;

class Manager {}

//- /src/Caller.php
<?php
namespace App;

use Dep\Container;

class Caller
{
    public function run(Container $container): void
    {
        $container->get('app.@here@manager');
    }
}
"#;

fn services(world: &mut phrost_harness::World) {
    world
        .contracts_mut()
        .bind_argument("Dep\\Container", "get", 0, "", "service");
    world
        .contracts_mut()
        .catalog_entry("service", "app.manager", Some("App\\Manager"));
}

#[test]
fn a_registered_id_is_the_class_it_names() {
    check_view_with(
        PROJECT,
        &[
            Query::cursor("what it is", "here"),
            Query::goto("where it goes", "here"),
        ],
        services,
        expect![[r#"
            what it is
            - catalog key app.manager is App\Manager
            where it goes
            - class App\Manager @ src/Manager.php 4:7-4:14
        "#]],
    );
}

/// An id nothing registered is nothing, which is the same answer an unbound call gets. A container
/// does not answer for a name it was never given.
#[test]
fn an_unregistered_id_is_nothing() {
    check_view_with(
        PROJECT,
        &[Query::cursor("unregistered", "here")],
        |world| {
            world
                .contracts_mut()
                .bind_argument("Dep\\Container", "get", 0, "", "service");
        },
        expect![[r#"
            unregistered
            - (nothing under the cursor)
        "#]],
    );
}

/// Without the binding the string is a string, and the cursor is on nothing.
#[test]
fn an_unbound_call_leaves_the_string_alone() {
    check_view_with(
        PROJECT,
        &[Query::cursor("unbound", "here")],
        |_| {},
        expect![[r#"
            unbound
            - (nothing under the cursor)
        "#]],
    );
}

/// A different method on the same class is not bound, so its string says nothing either.
#[test]
fn another_method_on_the_same_class_is_untouched() {
    check_view_with(
        r#"
//- /src/Container.php
<?php
namespace Dep;

class Container
{
    public function get(string $id): mixed
    {
        return NULL;
    }

    public function has(string $id): bool
    {
        return TRUE;
    }
}

//- /src/Manager.php
<?php
namespace App;

class Manager {}

//- /src/Caller.php
<?php
namespace App;

use Dep\Container;

class Caller
{
    public function run(Container $container): void
    {
        $container->has('app.@here@manager');
    }
}
"#,
        &[Query::cursor("other method", "here")],
        services,
        expect![[r#"
            other method
            - (nothing under the cursor)
        "#]],
    );
}

#[test]
fn the_cursor_answers_in_the_argument_the_rule_names_and_in_no_other() {
    // Both directions in one fixture, because each is the other's false positive. A cursor in the
    // *account* of `allowedIfHasPermission($account, $permission)` is reading a plain string, and so
    // is a cursor in the second argument of a method whose key really is the first -- which is what a
    // position read as "any argument" would have got wrong.
    check_view_with(
        r#"
//- /src/Dep.php
<?php
namespace Dep;

class Access
{
    public static function allowedIfHasPermission(string $account, string $permission): mixed
    {
        return NULL;
    }
}

class Container
{
    public function get(string $id, string $fallback = ''): mixed
    {
        return NULL;
    }
}

//- /src/Caller.php
<?php
namespace App;

use Dep\Access;
use Dep\Container;

class Caller
{
    public function reads(): void
    {
        Access::allowedIfHasPermission('a', 'access @here@content');
    }

    public function account(): void
    {
        Access::allowedIfHasPermission('access @second@content', 'b');
    }

    public function later(Container $container): void
    {
        $container->get('a', 'access @third@content');
    }
}
"#,
        &[
            Query::cursor("the permission, which is the second argument", "here"),
            Query::cursor("the account, which is not", "second"),
            Query::cursor("a later argument of a first-argument rule", "third"),
        ],
        |world| {
            world.contracts_mut().bind_argument(
                "Dep\\Access",
                "allowedIfHasPermission",
                1,
                "",
                "permission",
            );
            world
                .contracts_mut()
                .bind_argument("Dep\\Container", "get", 0, "", "service");
            world
                .contracts_mut()
                .catalog_entry("permission", "access content", None);
        },
        expect![[r#"
            the permission, which is the second argument
            - catalog key access content is registered ()
            the account, which is not
            - (nothing under the cursor)
            a later argument of a first-argument rule
            - (nothing under the cursor)
        "#]],
    );
}
