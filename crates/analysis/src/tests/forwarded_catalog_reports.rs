//! An id nothing declares, written one call before the method that reads it.
//!
//! `getStorage('no_such_entity_type')` is a fatal error the moment it runs and nothing else can
//! catch it: the string is not a name PHP has. It went unreported for a long time not because the
//! catalog was open but because *nothing could see the site* -- a report reached a catalog through
//! the argument a call was given or the value under an array key, and this id is written one call
//! earlier than the method that consumes it.
//!
//! Beside [`super::catalog_reports`] rather than in it: that file owns the gate on the catalog being
//! complete, and this one owns the shape the site is written in. The two fail for different reasons
//! and a fixture proving one proves nothing about the other.

use expect_test::{Expect, expect};
use phrost_harness::{Forwarded, World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{Severity, diagnostics};

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

//- /src/Node.php
<?php
namespace App;

class Node implements \Dep\EntityInterface {}

//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{
    public function run(ManagerInterface $manager): void
    {
        $manager->getStorage('node')->load(1);
        $manager->getStorage('no_such_entity_type')->load(1);
    }
}
"#;

/// The entity catalog with one id declared, reached only through the call before `load`.
fn entities(world: &mut World) {
    world.contracts_mut().forward_argument(
        "Dep\\StorageInterface",
        "load",
        "getStorage",
        "entity",
        Forwarded::Instance,
    );
    world
        .contracts_mut()
        .catalog_entry("entity", "node", Some("App\\Node"));
}

#[test]
fn an_id_a_complete_catalog_does_not_hold_is_reported() {
    check(
        FIXTURE,
        |world| {
            entities(world);
            world.contracts_mut().close_catalog("entity");
        },
        expect![[r#"
            11:30-11:51 error [unknown-catalog-key] Nothing registers the entity `no_such_entity_type`
        "#]],
    );
}

#[test]
fn the_same_id_is_not_reported_when_the_catalog_may_be_incomplete() {
    // The gate the whole family rests on, restated for this shape: an open catalog answers "not one
    // I can see", which is not something to tell anybody about.
    check(FIXTURE, entities, expect![""]);
}

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
