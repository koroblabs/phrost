//! An id nothing registers, and the one thing that makes reporting it safe.
//!
//! The gate is not the key being missing -- it is the catalog claiming to hold every key there is.
//! Both sides are tested, because the check is worth nothing without the gate and reports correct
//! code without it.
//!
//! And an *open* catalog has a third answer: a key that is a plausible misspelling of one it holds.
//! That is a weaker claim than "does not exist" and a warning rather than an error, and what makes it
//! safe is that a typo always nearly hits a real key while a key that is merely absent does not.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{Severity, diagnostics};

fn check(fixture: &str, state: impl FnOnce(&mut World), expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    state(&mut world);
    let world = world;
    let path = spec
        .files()
        .last()
        .expect("the fixture declares a file")
        .path()
        .to_owned();
    let file = world.file_id(&path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        diagnostics(&view)
            .into_iter()
            .map(|found| {
                format!(
                    "{} {} [{}] {}\n",
                    world.render_range(file, found.range),
                    match found.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                        Severity::Note => "note",
                    },
                    found.code,
                    found.message
                )
            })
            .collect::<String>()
    });

    expect.assert_eq(&rendered);
}

const FIXTURE: &str = r#"
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
    public function run(Container $container, string $chosen): void
    {
        $container->get('app.manager');
        $container->get('app.nothing');
        $container->get($chosen);
    }
}
"#;

fn registered(world: &mut World) {
    world
        .contracts_mut()
        .bind_argument("Dep\\Container", "get", 0, "", "service");
    world
        .contracts_mut()
        .catalog_entry("service", "app.manager", Some("App\\Manager"));
}

#[test]
fn an_id_a_complete_catalog_does_not_hold_is_reported() {
    check(
        FIXTURE,
        |world| {
            registered(world);
            world.contracts_mut().close_catalog("service");
        },
        expect![[r#"
            11:25-11:38 error [unknown-catalog-key] Nothing registers the service `app.nothing`
        "#]],
    );
}

#[test]
fn the_same_id_is_not_reported_when_the_catalog_may_be_incomplete() {
    // The whole gate. A service catalog reads every container file there is and still cannot see a
    // service a module registers in code, so a missing key there means "not one I can see", and
    // reporting it lights up correct code.
    check(FIXTURE, registered, expect![""]);
}

#[test]
fn a_registered_id_whose_class_is_unknowable_is_still_registered() {
    // A service built by a factory, or one whose class is a parameter: the configuration declares the
    // id and says nothing about what it builds. Collapsing the two made a declared service read as
    // undeclared -- found by this report, on real code, saying a logger channel did not exist.
    check(
        FIXTURE,
        |world| {
            registered(world);
            world.contracts_mut().close_catalog("service");
            world
                .contracts_mut()
                .catalog_entry("service", "app.nothing", None);
        },
        expect![""],
    );
}

/// An open catalog reports a near miss, and nothing else.
///
/// The two halves are one test on purpose: the same catalog, the same call, one key that looks like
/// a misspelling of a registered one and one that looks like nothing registered. Splitting them
/// would let the gate be removed and half the file still pass.
#[test]
fn an_open_catalog_reports_a_misspelling_and_not_an_absence() {
    check(
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
}

//- /src/Caller.php
<?php
namespace App;

use Dep\Container;

class Caller
{
    public function run(Container $container): void
    {
        $container->get('entity.node.canonical');
        $container->get('entity.node.canonicl');
        $container->get('view.frontpage.page_1');
        $container->get('Float');
    }
}
"#,
        |world| {
            world
                .contracts_mut()
                .bind_argument("Dep\\Container", "get", 0, "", "route");
            world
                .contracts_mut()
                .catalog_entry("route", "entity.node.canonical", None);
            world
                .contracts_mut()
                .catalog_entry("route", "app_core.settings", None);
        },
        expect![[r#"
            11:25-11:47 warning [unknown-catalog-key] No route is registered as `entity.node.canonicl`. Did you mean `entity.node.canonical`?
        "#]],
    );
}

/// A closed catalog says what it always said, and adds the suggestion when there is one.
#[test]
fn a_complete_catalog_names_the_key_it_was_probably_meant_to_be() {
    check(
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
}

//- /src/Caller.php
<?php
namespace App;

use Dep\Container;

class Caller
{
    public function run(Container $container): void
    {
        $container->get('app.mangaer');
    }
}
"#,
        |world| {
            registered(world);
            world.contracts_mut().close_catalog("service");
        },
        expect![[r#"
            10:25-10:38 error [unknown-catalog-key] Nothing registers the service `app.mangaer`. Did you mean `app.manager`?
        "#]],
    );
}

/// Both operands of a conditional are keys the call may be given, so both are read as uses.
///
/// Goto already said so -- the cursor counts which argument a string sits in rather than requiring
/// the argument to be the string -- and the list of uses did not, so `get($former ? 'a' : 'b')` was
/// a key to goto and to hover, and nothing to references, to a rename, or to this report. Found on
/// the reference project, where references from such a field name listed nothing at all.
#[test]
fn both_operands_of_a_conditional_argument_are_read_as_keys() {
    check(
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
}

//- /src/Caller.php
<?php
namespace App;

use Dep\Container;

class Caller
{
    public function run(Container $container, bool $former): void
    {
        $container->get($former ? 'app.manager' : 'app.nothing');
        $container->get($former ?: 'app.missing');
    }
}
"#,
        |world| {
            registered(world);
            world.contracts_mut().close_catalog("service");
        },
        expect![[r#"
            10:51-10:64 error [unknown-catalog-key] Nothing registers the service `app.nothing`
            11:36-11:49 error [unknown-catalog-key] Nothing registers the service `app.missing`
        "#]],
    );
}
