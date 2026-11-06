//! A key a report can see, written one call before the method that reads it.
//!
//! The other half of [`super::forwarded_catalog`], and a different question. That one asks what
//! `$storage->load($id)` gives back, which needs both calls; this asks whether `'node'` is an entity
//! type at all, which needs only the first -- an id nothing registers is wrong whether or not
//! anything is ever loaded from the storage it asked for.
//!
//! Owned here because the whole-file sweep is a projection, and because the gate that keeps it
//! honest lives here too: the call has to hand back the class the forwarding rules are written on.
//! A reader keyed on the method's *name* alone would report every project's own `getStorage`, which
//! against a closed catalog turns correct code into an error.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_index::Forwarded;
use phrost_testkit::FixtureSpec;

use crate::IndexView;

/// A storage handler reached through a manager, and a second `getStorage` that is not one.
///
/// `App\Cache` is the case the gate exists for: a real project is allowed to declare a method of
/// that name meaning something else, and nothing in its argument is an entity type id.
const FIXTURE: &str = r#"
//- /src/Storage.php
<?php
namespace Dep;

interface EntityInterface {}

interface StorageInterface
{
    /** @return \Dep\EntityInterface|null */
    public function load($id);
}

interface ManagerInterface
{
    /** @return \Dep\StorageInterface */
    public function getStorage($id);
}

//- /src/Entities.php
<?php
namespace App;

use Dep\EntityInterface;

class Node implements EntityInterface {}

class Bucket {}

class Cache
{
    /** @return \App\Bucket */
    public function getStorage($name)
    {
        return new Bucket();
    }
}
"#;

/// The entity catalog, closed, with one id in it.
fn entities(world: &mut World) {
    world.contracts_mut().forward_argument(
        "Dep\\StorageInterface",
        "load",
        "getStorage",
        "entity",
        Forwarded::Instance,
    );
    // A second rule on the same call, because five of them name `getStorage` in the real provider
    // and each would otherwise underline the same literal again.
    world.contracts_mut().forward_argument(
        "Dep\\StorageInterface",
        "loadMultiple",
        "getStorage",
        "entity",
        Forwarded::ManyOf,
    );
    world
        .contracts_mut()
        .catalog_entry("entity", "node", Some("App\\Node"));
    world.contracts_mut().close_catalog("entity");
}

#[test]
fn the_argument_to_the_producing_call_is_the_key() {
    // One line per use and one only, whichever of the five rules named the call.
    check_uses(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{{
    public function run(ManagerInterface $manager): void
    {{
        $manager->getStorage('node');
        $manager->getStorage('no_such_entity_type');
    }}
}}
"#
        ),
        entities,
        expect![[r#"
            - entity node: known, catalog closed
            - entity no_such_entity_type: unknown, catalog closed
        "#]],
    );
}

#[test]
fn a_method_of_the_same_name_returning_something_else_writes_no_key() {
    // The gate. `getStorage` is a name a project may declare for anything, and the rules are
    // written on what the call hands back -- so a `getStorage` returning a `Bucket` is somebody
    // else's method and its argument is nobody's catalog key.
    check_uses(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    public function run(Cache $cache): void
    {{
        $cache->getStorage('whatever_this_is');
    }}
}}
"#
        ),
        entities,
        expect![""],
    );
}

#[test]
fn a_key_that_is_not_a_literal_is_not_a_key_written_wrong() {
    // A computed id is a value, not a mistake: there is nothing written there to be right or wrong
    // about, and reporting it would report every project that keeps its ids in constants.
    check_uses(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{{
    public function run(ManagerInterface $manager, string $type): void
    {{
        $manager->getStorage($type);
        $manager->getStorage();
    }}
}}
"#
        ),
        entities,
        expect![""],
    );
}

#[test]
fn an_open_catalog_says_so_and_the_report_is_left_to_decide() {
    // Whether a miss is worth telling anybody is not this projection's decision. Both halves come
    // back, and the check above is what reads the second.
    check_uses(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{{
    public function run(ManagerInterface $manager): void
    {{
        $manager->getStorage('no_such_entity_type');
    }}
}}
"#
        ),
        |world| {
            world.contracts_mut().forward_argument(
                "Dep\\StorageInterface",
                "load",
                "getStorage",
                "entity",
                Forwarded::Instance,
            );
        },
        expect![[r#"
            - entity no_such_entity_type: unknown, catalog open
        "#]],
    );
}

/// Every catalog use the sweep finds in the last file of the fixture, whole.
///
/// The whole set rather than a lookup: an extra use is a report on code that is correct, so it has
/// to be as visible as a missing one.
fn check_uses(fixture: &str, state: impl FnOnce(&mut World), expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    state(&mut world);
    let world = world;

    let file = world.file_id("src/Caller.php");
    let answer = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let mut out = String::new();
        for used in view.catalog_uses() {
            let _ = writeln!(
                out,
                "- {} {}: {}, catalog {}",
                used.catalog,
                used.key,
                if used.known { "known" } else { "unknown" },
                if used.closed { "closed" } else { "open" }
            );
        }
        out
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&answer);
}
