//! What a string that indexes a project catalog offers.
//!
//! The one place completion happens *inside* a literal, and the reason is that the literal is the
//! only statement anywhere of what is legal there: `\Drupal::service('application.manager')` is
//! declared to return `object`, so the id carries the whole answer. Everything else inside a string
//! still gets nothing.

use expect_test::expect;

use crate::tests::{Query, check, check_with};

/// A container reached by an id offers the ids, filtered by what has been typed.
const CONTAINER: &str = r#"
//- /src/Container.php
<?php
namespace Dep;

class Container
{
    public function get(string $id): mixed
    {
        return NULL;
    }
}

//- /src/Caller.php
<?php
namespace App;

use Dep\Container;

class Caller
{
    public function run(Container $container): void
    {
        $container->get('app@here@');
    }
}
"#;

fn services(world: &mut phrost_harness::World) {
    world
        .contracts_mut()
        .bind_argument("Dep\\Container", "get", 0, "", "service");
    for (id, class) in [
        ("app.manager", "App\\Manager"),
        ("app.logger", "App\\Logger"),
        ("other.thing", "App\\Other"),
    ] {
        world
            .contracts_mut()
            .catalog_entry("service", id, Some(class));
    }
}

#[test]
fn a_bound_argument_offers_the_catalog_keys_that_match() {
    check_with(
        CONTAINER,
        &[Query::completion("ids", "here")],
        services,
        expect![[r#"
            ids
            - app.logger [catalog key]  App\Logger
            - app.manager [catalog key]  App\Manager
            - ... and the client must ask again
            - accepting the first leaves `$container->get('app.logger');`
        "#]],
    );
}

/// Without the binding there is no list, which is the same silence every other string gets.
#[test]
fn an_unbound_call_offers_nothing_inside_its_string() {
    check(
        CONTAINER,
        &[Query::completion("ids", "here")],
        expect![[r#"
            ids
            - (nothing offered)
        "#]],
    );
}

/// A string that is not the call's first argument is just a string.
#[test]
fn a_later_argument_offers_nothing() {
    check_with(
        r#"
//- /src/Container.php
<?php
namespace Dep;

class Container
{
    public function get(string $id, string $mode): mixed
    {
        return NULL;
    }
}

//- /src/Caller.php
<?php
namespace App;

use Dep\Container;

class Caller
{
    public function run(Container $container): void
    {
        $container->get('app.manager', 'ap@here@');
    }
}
"#,
        &[Query::completion("second argument", "here")],
        services,
        expect![[r#"
            second argument
            - (nothing offered)
        "#]],
    );
}

/// A string somewhere else entirely still gets nothing, which is what keeps a class list out of
/// prose and out of every array key in a file.
#[test]
fn a_string_outside_a_bound_call_offers_nothing() {
    check_with(
        r#"
//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): array
    {
        return ['ap@here@' => 1];
    }
}
"#,
        &[Query::completion("array key", "here")],
        services,
        expect![[r#"
            array key
            - (nothing offered)
        "#]],
    );
}
