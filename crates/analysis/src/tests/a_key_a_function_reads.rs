//! A key a free function reads out of a catalog: `view('posts.show')`, `app('router')`.
//!
//! Goto has gone from one to its file all along, by the function's binding; the list of a file's
//! catalog uses read only methods' bindings, so find-references and the highlight drawn at the same
//! key answered nothing. Both read one list now, and a key is every call that writes it and none
//! that writes another.

use std::fmt::Write as _;

use expect_test::expect;

const FIXTURE: &str = r#"
//- /src/Controller.php
<?php
namespace App;

function view(string $name) {}
function app(string $id) {}

class Controller
{
    public function show(): void
    {
        view('posts.sh@view@ow');
        view('posts.show');
        view('posts.index');
        app('rou@app@ter');
        app('router');
    }
}
"#;

fn rule(world: &mut phrost_harness::World) {
    let contracts = world.contracts_mut();
    contracts.bind_function_argument("view", 0, "views");
    contracts.bind_function_argument("app", 0, "container");
    contracts.catalog_entry("views", "posts.show", None);
    contracts.catalog_entry("views", "posts.index", None);
    contracts.catalog_entry("container", "router", None);
}

#[test]
fn references_list_every_call_writing_the_key() {
    let spec = phrost_testkit::FixtureSpec::parse(FIXTURE);
    let world = phrost_harness::World::build_with(&spec, rule);
    let mut out = String::new();
    for marker in ["view", "app"] {
        let position = spec.markers().position(marker);
        let file = world.file_id(&position.path);
        let open = world.open(phrost_harness::OpenScope::EveryFile, Some(file));
        let view = phrost_view::IndexView::in_file(world.skeleton(), &open, file);
        let target = crate::reference_target(&view, position.offset).expect("a key to search for");
        let _ = writeln!(out, "from {marker}:");
        for range in crate::references_in(&view, &target) {
            let _ = writeln!(out, "  {}", world.render_range(file, range));
        }
    }
    spec.markers().assert_all_used();
    expect![[r#"
        from view:
          11:15-11:25
          12:15-12:25
        from app:
          14:14-14:20
          15:14-15:20
    "#]]
    .assert_eq(&out);
}
