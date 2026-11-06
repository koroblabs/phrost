//! A call whose key is not its leading argument.
//!
//! Nearly every method that looks a value up by a string puts the string first, and the mechanism was
//! built on that: one rule per method, and the first literal is the key. `AccessResult::
//! allowedIfHasPermission($account, $permission)` is the exception, and it is not an obscure one --
//! it is how an access check is written in every controller and every access handler Drupal ships.
//!
//! What made it look impossible was a note saying the body layer carries only the leading literal.
//! That stopped being true when `PathStep::MethodReturn` grew a list of literals with their
//! positions, for the `class-string<T>` work; the rule simply had nowhere to say which one it meant.
//!
//! Both directions have to hold, and the second is the one that costs: the *account* must stay a
//! plain string, and so must every later argument of a method whose key really is the first. A
//! position that is read as "any argument" turns `get('a', 'b')` into two reports.

use expect_test::expect;

use super::diagnostics::check_framework;

const FIXTURE: &str = r#"
//- /src/Access.php
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
    public function run(Container $container): void
    {
        Access::allowedIfHasPermission('administer nodes', 'access content');
        Access::allowedIfHasPermission('administer nodes', 'acces content');
        $container->get('app.manager', 'access content');
    }
}
"#;

fn registered(world: &mut phrost_harness::World) {
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
    world.contracts_mut().close_catalog("permission");
    world
        .contracts_mut()
        .catalog_entry("service", "app.manager", Some("App\\Manager"));
    world.contracts_mut().close_catalog("service");
}

#[test]
fn the_argument_a_rule_names_is_the_key_and_the_ones_around_it_are_not() {
    // Three literals that would each be a permission if the position were ignored, and one report.
    // `'administer nodes'` is a real permission id written in the account's slot on purpose: it is
    // what a rule reading the first argument would have reported, twice, on correct code.
    check_framework(
        FIXTURE,
        registered,
        expect![[r#"
            12:60-12:75 error [unknown-catalog-key] Nothing registers the permission `acces content`. Did you mean `access content`?
        "#]],
    );
}
