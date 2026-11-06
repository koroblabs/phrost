//! A docblock a framework prints at somebody rather than storing.
//!
//! Drupal's update hooks are the case. What `update.php` shows beside a pending update, and what
//! `drush updb` lists before asking whether to run it, is the function's own docblock with the
//! stars stripped and the lines joined -- so the tags the documentation rules ask for become words
//! in the middle of a sentence a person reads while deciding whether to migrate a database.
//!
//! Two things are pinned here and the second is the one that could rot quietly. The rules that
//! demand a tag say nothing about such a declaration -- and they still speak about the ordinary
//! function written beside it in the same file, which is what tells an exemption from a rule that
//! stopped working. And `Write a docblock` still offers, because a hook with *no* docblock gets an
//! empty description; it writes the summary and stops.
//!
//! The fixtures are `.php` where a real module writes `.install`, and that costs the tests nothing:
//! neither registry checks the file. Drupal loads the update files and then matches over
//! `get_defined_functions()`, so the name is the whole of what says a declaration is one of these.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use super::diagnostics::check_asked_for_with;
use crate::code_actions;

/// An update hook and an ordinary function of the same module, in one file.
///
/// Both return a value and neither documents it, so the difference in what is reported is the whole
/// of what the rule contributes. `_update_status` is Drupal's own warning made into a control: its
/// name carries `_update_` and it is not an update.
const MODULE: &str = r#"
//- /modules/wh_item_snapshot/wh_item_snapshot.php
<?php

/**
 * Adds the coordinating exception flag to the item snapshot.
 */
function wh_item_snapshot_update_8001(): string {
    return 'Added the flag.';
}

/**
 * Backfills the flag for rows written before it existed.
 */
function wh_item_snapshot_post_update_backfill(array &$sandbox): string {
    return 'Backfilled.';
}

/**
 * Reads the current status.
 */
function wh_item_snapshot_update_status(int $id): string {
    return 'ok';
}
"#;

/// What Drupal contributes: the two name shapes whose docblock it publishes.
fn updates(world: &mut World) {
    world
        .contracts_mut()
        .add_user_facing_docblock("_update_", true);
    world
        .contracts_mut()
        .add_user_facing_docblock("_post_update_", false);
}

#[test]
fn the_rules_that_demand_a_tag_say_nothing_about_an_update_hook() {
    check_asked_for_with(
        MODULE,
        &["doc-missing-tags"],
        updates,
        expect![[r#"
            20:10-20:40 warning [doc-missing-tags] returns a value and has no @return
            20:45-20:48 warning [doc-missing-tags] $id has no @param
        "#]],
    );
}

#[test]
fn without_the_contract_every_one_of_them_is_reported() {
    // The whole exemption arrives from the seam: a project with no framework hears about all three,
    // which is also what says the fixture would otherwise fire.
    check_asked_for_with(
        MODULE,
        &["doc-missing-tags"],
        |_| {},
        expect![[r#"
            6:10-6:38 warning [doc-missing-tags] returns a value and has no @return
            13:10-13:47 warning [doc-missing-tags] returns a value and has no @return
            13:55-13:63 warning [doc-missing-tags] $sandbox has no @param
            20:10-20:40 warning [doc-missing-tags] returns a value and has no @return
            20:45-20:48 warning [doc-missing-tags] $id has no @param
        "#]],
    );
}

/// A `@throws` is published too, so the rule about one is off for the same reason.
///
/// The exception is declared in the fixture rather than named from the standard library: a fixture
/// has no stubs, so a `\RuntimeException` resolves to nothing, `thrown_classes` drops it, and both
/// halves of the pair below report an `undefined-type` and agree. That is what the first accepted
/// snapshot said, and it read as the exemption working.
const THROWING: &str = r#"
//- /modules/wh_item_snapshot/wh_item_snapshot.php
<?php

class UpdateFailed {}

/**
 * Adds the coordinating exception flag to the item snapshot.
 */
function wh_item_snapshot_update_8001(): void {
    throw new UpdateFailed('no');
}
"#;

#[test]
fn an_undocumented_throw_in_an_update_hook_is_not_reported_either() {
    check_asked_for_with(THROWING, &["doc-missing-throws"], updates, expect![[r#""#]]);
}

#[test]
fn and_is_reported_without_the_contract() {
    check_asked_for_with(
        THROWING,
        &["doc-missing-throws"],
        |_| {},
        expect![[r#"
        9:5-9:33 warning [doc-missing-throws] throws \UpdateFailed, which the docblock does not declare
    "#]],
    );
}

/// The docblock the action titled `title` leaves behind, or what it offered instead.
fn action(
    fixture: &str,
    marker: &str,
    title: &str,
    state: impl FnOnce(&mut World),
    expect: Expect,
) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build_with(&spec, state);
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let request = crate::ActionRequest {
            view: &view,
            at: phrost_model::TextRange::empty(position.offset),
            only: &[],
            asked: Default::default(),
        };
        let offered = code_actions(&request);
        let Some(action) = offered.iter().find(|action| action.title == title) else {
            return format!(
                "(not offered; offered {:?})\n",
                offered.iter().map(|a| &a.title).collect::<Vec<_>>()
            );
        };
        let mut text = world.text_of(file).to_owned();
        let mut edits = action.edits.clone();
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
        for edit in edits {
            let range = usize::from(edit.range.start())..usize::from(edit.range.end());
            text.replace_range(range, &edit.text);
        }
        let open = text.find("/**").expect("a docblock was written");
        let close = text[open..].find("*/").expect("the docblock is closed") + open + 2;
        format!("{}\n", &text[open..close])
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

/// An update hook with no docblock at all, which is the one that gets an empty description.
const UNDOCUMENTED: &str = r#"
//- /modules/wh_item_snapshot/wh_item_snapshot.php
<?php

function wh_item_snapshot_$0update_8001(array &$sandbox): string {
    return 'Added the flag.';
}
"#;

#[test]
fn writing_one_stops_at_the_summary_and_leaves_the_line_to_overwrite() {
    // Not a summary derived from the name: `wh_item_snapshot_update_8001` yields "Lvs item snapshot
    // update 8001.", which is a fine suggestion in an editor and not a thing to print beside a
    // pending migration.
    action(
        UNDOCUMENTED,
        "0",
        "Write a docblock",
        updates,
        expect![[r#"
            /**
             * Describe what this does.
             */
        "#]],
    );
}

/// An update hook whose docblock is missing the `@param` and the `@return` for its signature.
const INCOMPLETE: &str = r#"
//- /modules/wh_item_snapshot/wh_item_snapshot.php
<?php

/**
 * Adds the coordinating exception flag to the item snapshot.
 */
function wh_item_snapshot_$0update_8001(array &$sandbox): string {
    return 'Added the flag.';
}
"#;

#[test]
fn nothing_offers_to_add_the_tag_it_is_missing() {
    action(
        INCOMPLETE,
        "0",
        "Update the docblock",
        updates,
        expect![[r#"
            (not offered; offered [])
        "#]],
    );
}

#[test]
fn and_the_same_position_offers_it_without_the_contract() {
    // Which is the half that says the test above is about the exemption. `Write a docblock` is not
    // offered at either -- the declaration already has one -- so an empty offer list there would
    // have read the same whether the action had been withdrawn or had never been there.
    action(
        INCOMPLETE,
        "0",
        "Update the docblock",
        |_| {},
        expect![[r#"
            /**
             * Adds the coordinating exception flag to the item snapshot.
             * @param array $sandbox The sandbox.
             * @return string The string.
             */
        "#]],
    );
}
