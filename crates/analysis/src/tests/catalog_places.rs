//! A string that names something declared outside PHP.
//!
//! A route name and a permission id are the two of these a real project writes most -- 106 and 141
//! on the one measured here. Neither is a name PHP has, so before this the string was the one thing
//! at that position with no meaning at all: nothing to offer, nowhere to go, no way to tell a typo
//! from a route.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
// Through the view, not from the index: this crate may not name a store crate, and the check that
// says so is what caught the direct import.
use phrost_view::{IndexView, Place};
use std::path::Path;
use std::sync::Arc;

use crate::{completions, goto_external};

/// Two routes and a service, so a class-valued key and a place-valued one are in play at once.
fn with_places(world: &mut World) {
    world
        .contracts_mut()
        .bind_argument("Dep\\Links", "toRoute", 0, "", "route");
    for (key, line, detail) in [
        ("app.edit", 12_u32, "/app/{id}/edit"),
        ("app.delete", 20, "/app/{id}/delete"),
    ] {
        world.contracts_mut().catalog_place(
            "route",
            key,
            Place {
                path: Arc::from(Path::new("/project/app.routing.yml")),
                line,
                character: 0,
                detail: detail.into(),
            },
        );
    }
}

const FIXTURE: &str = r#"
//- /src/Links.php
<?php
namespace Dep;

class Links
{
    public static function toRoute(string $name): string
    {
        return '';
    }
}
"#;

fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    with_places(&mut world);
    let world = world;
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let mut out = String::new();
        let list = completions(&view, position.offset);
        if list.items.is_empty() {
            out.push_str("offered: (nothing)\n");
        }
        for item in &list.items {
            out.push_str(&format!(
                "offered: {}  {}\n",
                item.label,
                item.detail.as_deref().unwrap_or("")
            ));
        }
        match goto_external(&view, position.offset) {
            Some(target) => out.push_str(&format!(
                "goes to: {}:{}:{} as `{}`\n",
                target.path.display(),
                target.line + 1,
                target.character + 1,
                target.key
            )),
            None => out.push_str("goes to: (nowhere outside the project)\n"),
        }
        out
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn a_prefix_offers_the_keys_with_what_each_one_serves() {
    // The description is the point. A hundred route names with nothing beside them is a list nobody
    // can choose from; a hundred names each beside its URL is one anybody can.
    check(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Links;

class Caller
{{
    public function run(): void
    {{
        Links::toRoute('app.@here@');
    }}
}}
"#
        ),
        "here",
        expect![[r#"
            offered: app.delete  /app/{id}/delete
            offered: app.edit  /app/{id}/edit
            goes to: (nowhere outside the project)
        "#]],
    );
}

#[test]
fn the_whole_key_goes_to_where_it_is_declared() {
    check(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Links;

class Caller
{{
    public function run(): void
    {{
        Links::toRoute('app.e@here@dit');
    }}
}}
"#
        ),
        "here",
        expect![[r#"
            offered: app.edit  /app/{id}/edit
            offered: app.delete  /app/{id}/delete
            goes to: /project/app.routing.yml:13:1 as `app.edit`
        "#]],
    );
}

#[test]
fn a_key_nothing_declares_goes_nowhere() {
    // The honest answer for a typo, and the reason a jump cannot be a guess: the nearest declared
    // route is not the one that was meant, it is the one that would hide the mistake.
    check(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Links;

class Caller
{{
    public function run(): void
    {{
        Links::toRoute('app.nothing_like_it@here@');
    }}
}}
"#
        ),
        "here",
        expect![[r#"
            offered: (nothing)
            goes to: (nowhere outside the project)
        "#]],
    );
}
