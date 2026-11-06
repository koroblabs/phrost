//! A search at a catalog key lists the other places that read that key.
//!
//! `\Drupal::service('app_signing.package')` names a class for goto, and a search there used to list
//! where that *class* is written -- its imports and type hints -- and never the other places asking
//! for the service, which is what a person standing on an id wants. The key is the target now, and
//! which catalog it is a key of is part of it: a route and a service may share a spelling.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{reference_target, references_in, rename_refusal};

const PROJECT: &str = r#"
//- /src/Container.php
<?php
namespace Dep;

class Container
{
    public function get(string $id): mixed {}
    public function url(string $route): string {}
}

//- /src/Manager.php
<?php
namespace App;

class Manager {}

//- /src/First.php
<?php
namespace App;

use Dep\Container;

class First
{
    public function run(Container $container): void
    {
        $container->get('app.ma@here@nager');
        $container->get('app.other');
        $container->url('app.manager');
    }
}

//- /src/Second.php
<?php
namespace App;

use Dep\Container;

function second(Container $container): void
{
    $container->get('app.manager');
}
"#;

fn bindings(world: &mut World) {
    let contracts = world.contracts_mut();
    contracts.bind_argument("Dep\\Container", "get", 0, "", "service");
    contracts.bind_argument("Dep\\Container", "url", 0, "", "route");
    contracts.catalog_entry("service", "app.manager", Some("App\\Manager"));
    contracts.catalog_entry("service", "app.other", Some("App\\Manager"));
    contracts.catalog_entry("route", "app.manager", None);
}

fn check(expect: Expect) {
    let spec = FixtureSpec::parse(PROJECT);
    let world = World::build_with(&spec, bindings);
    let position = spec.markers().position("here");
    let cursor = world.file_id(&position.path);
    let open = world.open(OpenScope::EveryFile, Some(cursor));
    let mut out = String::new();

    let target = reference_target(
        &IndexView::in_file(world.skeleton(), &open, cursor),
        position.offset,
    )
    .expect("the cursor is on a key");
    let _ = writeln!(out, "{target:?}");
    for file in spec.files() {
        let id = world.file_id(file.path());
        let view = IndexView::in_file(world.skeleton(), &open, id);
        for range in references_in(&view, &target) {
            let _ = writeln!(out, "  {} {}", file.path(), world.render_range(id, range));
        }
    }
    let view = IndexView::in_file(world.skeleton(), &open, cursor);
    let _ = writeln!(
        out,
        "rename: {}",
        rename_refusal(&view, &target, Some("app.renamed")).unwrap_or_default()
    );

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

#[test]
fn a_service_id_lists_where_it_is_read_and_nothing_else() {
    check(expect![[r#"
        CatalogKey { catalog: "service", key: "app.manager" }
          src/First.php 10:26-10:37
          src/Second.php 8:22-8:33
        rename: `app.manager` is declared in the framework's configuration, which a rename here does not edit -- rename it there and at its uses together
    "#]]);
}
