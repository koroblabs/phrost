//! `actions::inheritdoc`: offering `{@inheritdoc}` beside `document::WriteDocblock`.
//!
//! The whole inserted text is snapshotted, same reason as `tests::docblocks`: the layout is the
//! feature.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

/// The action titled `title`, as the text it would insert, with the file's own indentation shown.
fn check(fixture: &str, marker: &str, title: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

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
        action
            .edits
            .iter()
            .map(|edit| edit.text.replace("    ", "····"))
            .collect::<String>()
            + "<>\n"
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

/// Every title on offer at the marker, in the order `code_actions` produced them, each with
/// whether it is `preferred`.
fn titles(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let at = phrost_model::TextRange::empty(position.offset);
        let request = crate::ActionRequest {
            view: &view,
            at,
            only: &[],
            asked: Default::default(),
        };
        let offered = code_actions(&request);
        if offered.is_empty() {
            return "- (nothing on offer)\n".to_owned();
        }
        offered
            .iter()
            .map(|action| {
                format!(
                    "- {}{}\n",
                    action.title,
                    if action.preferred {
                        "  [preferred]"
                    } else {
                        ""
                    }
                )
            })
            .collect::<String>()
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn a_method_overriding_a_documented_interface_offers_inheritdoc() {
    check(
        r#"
//- /src/HasName.php
<?php
namespace App;

interface HasName
{
    /**
     * Gets the name.
     */
    public function getName(): string;
}

//- /src/Thing.php
<?php
namespace App;

class Thing implements HasName
{
    public function get@here@Name(): string
    {
        return '';
    }
}
"#,
        "here",
        "Document with {@inheritdoc}",
        expect![[r#"
            /**
            ···· * {@inheritdoc}
            ···· */
            ····<>
        "#]],
    );
}

#[test]
fn a_method_overriding_nothing_offers_no_inheritdoc() {
    check(
        r#"
//- /src/Thing.php
<?php
namespace App;

class Thing
{
    public function get@here@Name(): string
    {
        return '';
    }
}
"#,
        "here",
        "Document with {@inheritdoc}",
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn write_a_docblock_stands_beside_it_and_neither_is_alone() {
    // Both are correct and nothing here can tell which one a person wants, so **neither** is
    // preferred: a client applying "the" fix without asking would otherwise pick between two
    // things somebody meant to read.
    //
    // This test was written when `WriteDocblock` did mark itself preferred, to record that it had
    // no way to know this offer existed. It now asks the same question this one does, and this
    // snapshot is what said so.
    titles(
        r#"
//- /src/HasName.php
<?php
namespace App;

interface HasName
{
    /**
     * Gets the name.
     */
    public function getName(): string;
}

//- /src/Thing.php
<?php
namespace App;

class Thing implements HasName
{
    public function get@here@Name(): string
    {
        return '';
    }
}
"#,
        "here",
        expect![[r#"
            - Write a docblock
            - Document with {@inheritdoc}
        "#]],
    );
}
