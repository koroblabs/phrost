//! `'App\Repo::find'` -- PHP's string spelling of a static callable, as `add_action()`, `usort()`
//! and a deprecation notice write it.
//!
//! Goto from the string reached the method; a search or a rename from the method did not list the
//! string, so a rename left a hook calling a method that no longer exists. The method is matched
//! through the class the string names, inherited declarations included, and a name the class does
//! not have is prose.

use std::fmt::Write as _;

use expect_test::expect;

const FIXTURE: &str = r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public static function fi@here@nd(): void {}
}

class Child extends Repo {}

class Hooks
{
    public function register(): void
    {
        Repo::find();
        add('App\Repo::find');
        add('\App\Child::find');
        add('App\Repo::gone');
        add('App\Missing::find');
    }
}

function add(string $callback): void {}
"#;

#[test]
fn a_search_and_a_rename_from_the_method_reach_the_string() {
    let spec = phrost_testkit::FixtureSpec::parse(FIXTURE);
    let world = phrost_harness::World::build(&spec);
    let position = spec.markers().position("here");
    let file = world.file_id(&position.path);
    let open = world.open(phrost_harness::OpenScope::EveryFile, Some(file));
    let view = phrost_view::IndexView::in_file(world.skeleton(), &open, file);

    let mut out = String::new();
    for (asked, target) in [
        ("search", crate::reference_target(&view, position.offset)),
        ("rename", crate::rename_target(&view, position.offset)),
    ] {
        for range in crate::references_in(&view, &target.expect("a method")) {
            let _ = writeln!(
                out,
                "{asked} {} {}",
                world.render_range(file, range),
                &world.text_of(file)[range]
            );
        }
    }
    spec.markers().assert_all_used();
    expect![[r#"
        search 6:28-6:32 find
        search 15:15-15:19 find
        search 16:24-16:28 find
        search 17:26-17:30 find
        rename 6:28-6:32 find
        rename 15:15-15:19 find
        rename 16:24-16:28 find
        rename 17:26-17:30 find
    "#]]
    .assert_eq(&out);
}

/// The dialogue opens on the string's method, which the rename edits.
#[test]
fn the_rename_dialogue_opens_on_the_string() {
    expect![[r#"
        16:24-16:28 find
    "#]]
    .assert_eq(&span(
        &FIXTURE
            .replace("fi@here@nd", "find")
            .replace("add('App\\Repo::find')", "add('App\\Repo::fi@here@nd')"),
        "here",
    ));
}

/// Where the rename dialogue opens for the marked cursor -- which must be where the rename edits.
fn span(fixture: &str, marker: &str) -> String {
    let spec = phrost_testkit::FixtureSpec::parse(fixture);
    let world = phrost_harness::World::build(&spec);
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);
    let open = world.open(phrost_harness::OpenScope::EveryFile, Some(file));
    let view = phrost_view::IndexView::in_file(world.skeleton(), &open, file);
    spec.markers().assert_all_used();
    match crate::rename_span(&view, position.offset) {
        Some(range) => {
            let range = crate::rename_range(world.text_of(file), range);
            format!(
                "{} {}\n",
                world.render_range(file, range),
                &world.text_of(file)[range]
            )
        }
        None => "the dialogue does not open\n".to_owned(),
    }
}
