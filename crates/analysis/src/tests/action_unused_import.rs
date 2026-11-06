//! `actions::unused_import`: deleting a `use` line nothing in the file names.
//!
//! The snapshot is the line the file reads after the edit, not the edit itself -- reading a range
//! and a string says nothing about whether removing it leaves the next statement exactly where it
//! was.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

/// What the action titled `title` would leave the line at its edit looking like.
fn check(fixture: &str, marker: &str, title: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);
    let text = world.text_of(file);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let at = phrost_model::TextRange::empty(position.offset);
        let request = crate::ActionRequest {
            view: &view,
            at,
            only: &[],
            asked: Default::default(),
        };
        let Some(action) = code_actions(&request)
            .into_iter()
            .find(|action| action.title == title)
        else {
            return "(nothing offered)\n".to_owned();
        };

        let mut out = String::new();
        for edit in &action.edits {
            let start = usize::from(edit.range.start());
            let end = usize::from(edit.range.end());
            let applied = format!("{}{}{}", &text[..start], edit.text, &text[end..]);
            let at = applied[..start + edit.text.len()]
                .rfind('\n')
                .map_or(0, |at| at + 1);
            let to = applied[at..].find('\n').map_or(applied.len(), |to| at + to);
            out.push_str(&format!("leaves `{}`\n", applied[at..to].trim()));
        }
        out
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn a_standalone_unused_import_is_removed_whole_line_and_all() {
    check(
        r#"
//- /src/Caller.php
<?php
namespace App;

use App\Entity\No@here@de;
class Caller
{
    public function run(): void
    {
    }
}
"#,
        "here",
        "Remove unused import App\\Entity\\Node",
        expect!["leaves `class Caller`\n"],
    );
}

#[test]
fn an_aliased_unused_import_takes_its_alias_with_it() {
    check(
        r#"
//- /src/Caller.php
<?php
namespace App;

use App\Entity\No@here@de as Foo;
class Caller
{
    public function run(): void
    {
    }
}
"#,
        "here",
        "Remove unused import App\\Entity\\Node",
        expect!["leaves `class Caller`\n"],
    );
}

#[test]
fn a_grouped_unused_import_is_left_for_a_person_to_edit() {
    // `unused.range` is the tail alone for a grouped entry -- `Node` between the braces -- so it
    // never spans the same text as the fully qualified path, and that mismatch is what this
    // refuses on. Touching the group without knowing how PHP requires the braces to be closed up
    // is exactly the kind of edit this offer exists to never make.
    check(
        r#"
//- /src/Caller.php
<?php
namespace App;

use App\Entity\{No@here@de};

class Caller
{
    public function run(): void
    {
    }
}
"#,
        "here",
        "Remove unused import App\\Entity\\Node",
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_later_name_in_a_flat_list_is_left_for_a_person_to_edit() {
    // Only the first name in `use A, B;` sits directly after the keyword; `B` sits after `, `
    // instead, so the left-hand check this offer runs never matches and it stays silent rather
    // than guess where the comma belongs once `B` is gone.
    check(
        r#"
//- /src/Caller.php
<?php
namespace App;

use App\Entity\Node, App\Other\Th@here@ing;

class Caller
{
    public function run(): void
    {
    }
}
"#,
        "here",
        "Remove unused import App\\Other\\Thing",
        expect!["(nothing offered)\n"],
    );
}
