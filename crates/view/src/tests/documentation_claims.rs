//! Which declarations a docblock convention binds, and whether dispatch can reach them.
//!
//! The rule under test is the one thing here that could report correct code: a hook name carries
//! uppercase placeholders, and an implementation of `hook_form_FORM_ID_alter` is named nothing like
//! it. So every claim in the fixture is rendered with its verdict rather than only the unreachable
//! ones -- an answer that reported nothing would otherwise be indistinguishable from a projection
//! that read nothing at all.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};

use crate::IndexView;

/// Every claim the file makes, with what it names and whether anything can call it.
fn check(fixture: &str, extensions: &[&str], expect: Expect) {
    let mut world = World::parse(fixture);
    world.contracts_mut().add_documented_elsewhere(
        "Implements",
        "hook_",
        Some("extension"),
        Some("hook"),
    );
    for extension in extensions {
        world
            .contracts_mut()
            .catalog_entry("extension", extension, None);
    }

    let world = world;
    let file = world.file_id("app.php");
    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let found = view
            .documentation_claims()
            .iter()
            .map(|claim| {
                format!(
                    "{} {} implements {} -- {}, {}\n",
                    world.render_range(file, claim.name_range),
                    claim.name,
                    claim.implements,
                    if claim.reachable {
                        "reachable"
                    } else {
                        "nothing will call it"
                    },
                    if claim.exists {
                        "which exists"
                    } else {
                        "which nothing declares or dispatches"
                    }
                )
            })
            .collect::<String>();

        if found.is_empty() {
            "<no claim>\n".to_owned()
        } else {
            found
        }
    });

    expect.assert_eq(&rendered);
}

#[test]
fn an_uppercase_run_stands_for_one_or_more_parts_of_the_name() {
    // Every placeholder shape the 333 hooks of one real checkout carry, in the four spellings that
    // matter: leading, trailing, in the middle, and one run made of two capitalised words.
    check(
        r#"
//- /app.php
<?php

/**
 * Implements hook_form_FORM_ID_alter().
 */
function app_core_form_views_exposed_form_alter(): void {}

/**
 * Implements hook_ENTITY_TYPE_delete().
 */
function app_core_user_delete(): void {}

/**
 * Implements hook_preprocess_HOOK().
 */
function app_core_preprocess_menu__toolbar__gin(): void {}

/**
 * Implements hook_update_N().
 */
function app_core_update_10201(): void {}
"#,
        &["app_core"],
        expect![[r#"
            6:10-6:48 app_core_form_views_exposed_form_alter implements hook_form_FORM_ID_alter -- reachable, which nothing declares or dispatches
            11:10-11:30 app_core_user_delete implements hook_ENTITY_TYPE_delete -- reachable, which nothing declares or dispatches
            16:10-16:48 app_core_preprocess_menu__toolbar__gin implements hook_preprocess_HOOK -- reachable, which nothing declares or dispatches
            21:10-21:31 app_core_update_10201 implements hook_update_N -- reachable, which nothing declares or dispatches
        "#]],
    );
}

#[test]
fn the_owner_is_an_installed_extension_and_not_whatever_precedes_the_hooks_name() {
    // The seam between the owner and the hook falls where the catalog says, not at the first
    // underscore that makes the rest fit: `app_region` is not installed, so nothing owns
    // `app_region_notifications_alter` -- and the same name is fine once `app_region` is.
    let fixture = r#"
//- /app.php
<?php

/**
 * Implements hook_notifications_alter().
 */
function app_region_notifications_alter(): void {}
"#;

    check(
        fixture,
        &["app_region_notice"],
        expect![[r#"
            6:10-6:40 app_region_notifications_alter implements hook_notifications_alter -- nothing will call it, which nothing declares or dispatches
        "#]],
    );
    check(
        fixture,
        &["app_region_notice", "app_region"],
        expect![[r#"
            6:10-6:40 app_region_notifications_alter implements hook_notifications_alter -- reachable, which nothing declares or dispatches
        "#]],
    );
}

#[test]
fn a_claim_a_hole_cannot_be_stretched_to_cover_is_unreachable() {
    // A hole stands for at least one part, so the trailing `_alter` still has to be there -- which is
    // what keeps `hook_preprocess_HOOK` from matching every name that begins `preprocess`.
    check(
        r#"
//- /app.php
<?php

/**
 * Implements hook_preprocess_HOOK().
 */
function gin_page_attachments_alter(): void {}

/**
 * Implements hook_form_FORM_ID_alter().
 */
function gin_form_alter(): void {}
"#,
        &["gin"],
        expect![[r#"
            6:10-6:36 gin_page_attachments_alter implements hook_preprocess_HOOK -- nothing will call it, which nothing declares or dispatches
            11:10-11:24 gin_form_alter implements hook_form_FORM_ID_alter -- nothing will call it, which nothing declares or dispatches
        "#]],
    );
}

#[test]
fn only_the_opening_sentence_of_a_docblock_is_a_claim() {
    check(
        r#"
//- /app.php
<?php

/**
 * Rebuilds the report cache.
 *
 * Implements hook_form_alter() is not what this says.
 *
 * @see hook_theme()
 */
function app_core_rebuild_reports(): void {}

/**
 * Implementsomething written as one word.
 */
function app_core_other(): void {}
"#,
        &["app_core"],
        expect![["<no claim>\n"]],
    );
}

#[test]
fn without_a_convention_nothing_is_a_claim() {
    let world = World::parse(
        r#"
//- /app.php
<?php

/**
 * Implements hook_form_alter().
 */
function app_region_notifications_alter(): void {}
"#,
    );
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        format!("{}\n", view.documentation_claims().len())
    });

    expect!["0\n"].assert_eq(&rendered);
}
