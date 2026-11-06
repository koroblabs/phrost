//! `Implements custom hook_report_types_info().`
//!
//! A project's own hook is written that way -- three times on one real checkout -- and the claim
//! used to be read only where the name followed the lead immediately, so those docblocks were
//! ordinary prose to phrost. Which had a cost beyond a missed hover: the rules that ask for a
//! `@param` and a `@return` spoke about them, the action offered to write both, and phpcs then
//! reported the tags as duplicating the hook's own documentation
//! (`Drupal.Commenting.HookComment.HookReturnDoc`). Our answer generating another tool's error.
//!
//! coder's own sniff anchors the *end* of the line -- ` hook_x().` -- and does not care what sits
//! between, so this is the two tools reading one sentence the same way rather than a widening.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use super::diagnostics::check_asked_for_with;
use super::{Query, check_with};
use crate::code_actions;

/// The hook, and an implementation whose docblock puts a word before the name.
const PROJECT: &str = r#"
//- /core/system.api.php
<?php

/**
 * Declares the report types a module provides.
 *
 * @return array
 *   A list of report type class names.
 */
function hook_report_types_info(): array {
  return [];
}

//- /modules/app_sanctions/app_sanctions.php
<?php

/**
 * Implements custom hook_report_ty@reference@pes_info().
 */
function app_sanctions_repo$0rt_types_info(): array {
  return ['SanctionsReport'];
}
"#;

/// Which words open a claim and what the name it makes looks like are the framework's to say.
fn conventions(world: &mut World) {
    world.contracts_mut().add_documented_elsewhere(
        "Implements",
        "hook_",
        Some("extension"),
        Some("hook"),
    );
    world
        .contracts_mut()
        .catalog_entry("extension", "app_sanctions", None);
}

#[test]
fn the_rules_that_demand_a_tag_say_nothing_about_it() {
    check_asked_for_with(
        PROJECT,
        &["doc-missing-tags"],
        conventions,
        expect![[r#""#]],
    );
}

#[test]
fn without_the_convention_the_return_is_reported() {
    // The exemption arrives from the seam, and this is what says the fixture would otherwise fire
    // -- which is the half that could rot quietly.
    check_asked_for_with(
        PROJECT,
        &["doc-missing-tags"],
        |_| {},
        expect![[r#"
            6:10-6:41 warning [doc-missing-tags] returns a value and has no @return
        "#]],
    );
}

#[test]
fn the_sentence_still_leads_to_the_hook() {
    // The other reader of the same rule, and the reason the claim is worth recognising at all: the
    // twenty lines saying what the hook returns are written once, on the hook.
    check_with(
        PROJECT,
        &[
            Query::hover("hover on the implementation", "0"),
            Query::goto("goto from the sentence", "reference"),
        ],
        conventions,
        expect![[r#"
            hover on the implementation
            - code: <?php
            function app_sanctions_report_types_info(): array
            - ---
            - doc: Implements custom hook_report_types_info().
            - ---
            - note: documentation below is from hook_report_types_info()
            - doc: Declares the report types a module provides.
            - tag: Returns array A list of report type class names.
            goto from the sentence
            - function hook_report_types_info @ core/system.api.php 9:10-9:32 (full 9:1-11:2)
        "#]],
    );
}

/// Whether the action titled `title` is on offer where the marker stands, and what else is.
fn offers(
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
        match offered.iter().any(|action| action.title == title) {
            true => format!("{title} is offered\n"),
            false => format!(
                "{title} is not offered; offered {:?}\n",
                offered
                    .iter()
                    .map(|action| &action.title)
                    .collect::<Vec<_>>()
            ),
        }
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

/// The same project with only the marker this asks about: a fixture's markers all have to be
/// consumed, and the sentence's own is the other test's.
const IMPLEMENTATION: &str = r#"
//- /core/system.api.php
<?php

/**
 * Declares the report types a module provides.
 *
 * @return array
 *   A list of report type class names.
 */
function hook_report_types_info(): array {
  return [];
}

//- /modules/app_sanctions/app_sanctions.php
<?php

/**
 * Implements custom hook_report_types_info().
 */
function app_sanctions_repo$0rt_types_info(): array {
  return ['SanctionsReport'];
}
"#;

/// The half the report was about: the action wrote the tag that phpcs then reported.
///
/// Standing on the name, which is where every one of these offers sits. The whole offered list is
/// rendered rather than a boolean, because "not offered" has to be told from "nothing is offered
/// here at all", and the second would pass a test about an exemption while proving nothing.
#[test]
fn and_nothing_offers_to_write_the_tag_the_sniff_reports() {
    offers(
        IMPLEMENTATION,
        "0",
        "Add @return",
        conventions,
        expect![[r#"
            Add @return is not offered; offered []
        "#]],
    );
}

#[test]
fn and_offers_it_without_the_convention() {
    // The pair is what makes the empty list above mean something: the offer is there to be
    // withdrawn, and an exemption tested against a position that offers nothing anyway would pass
    // while proving nothing.
    offers(
        IMPLEMENTATION,
        "0",
        "Add @return",
        |_| {},
        expect![[r#"
        Add @return is offered
    "#]],
    );
}
