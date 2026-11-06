//! A docblock claiming an implementation the runtime will never dispatch to.
//!
//! Most of these are about the silence. A framework whose hook names carry placeholders --
//! `hook_form_FORM_ID_alter`, `hook_ENTITY_TYPE_delete`, `hook_preprocess_HOOK` -- has correct
//! implementations whose names look nothing like the hook they name, and a check that reported them
//! would be reporting the standard.

use expect_test::expect;
use phrost_harness::World;

use super::diagnostics::{check, check_framework};

/// A project whose framework dispatches by name, with four extensions installed.
fn conventions(world: &mut World) {
    world.contracts_mut().add_documented_elsewhere(
        "Implements",
        "hook_",
        Some("extension"),
        Some("hook"),
    );
    for extension in ["app_core", "app_region_notice", "wh_order_reports", "gin"] {
        world
            .contracts_mut()
            .catalog_entry("extension", extension, None);
    }
}

#[test]
fn a_name_no_installed_extension_could_own() {
    // The shape found on a real checkout: the module is `app_region_notice`, the function is
    // `app_region_notifications_alter`, and `app_region` is not an extension. Drupal looks for
    // `app_region_notice_notifications_alter`, finds nothing, and the notification types the function
    // returns are never registered. Nothing else in the project notices.
    check_framework(
        r#"
//- /modules/app_core/app_core.api.php
<?php
function hook_notifications_alter(): array {
    return [];
}

//- /modules/app_region_notice/app_region_notice.php
<?php

/**
 * Implements hook_notifications_alter().
 */
function app_region_notifications_alter(): array {
    return [];
}
"#,
        conventions,
        expect![[r#"
            6:10-6:40 warning [misnamed-implementation] app_region_notifications_alter() is documented as implementing hook_notifications_alter() and is not named for it, so nothing will call it
        "#]],
    );
}

#[test]
fn a_docblock_copied_onto_a_function_that_implements_something_else() {
    // The commonest way this happens: the function below was written by copying the one above it.
    check_framework(
        r#"
//- /modules/app_core/app_core.api.php
<?php
function hook_preprocess_HOOK(array &$variables): void {
}

//- /modules/gin/gin.php
<?php

/**
 * Implements hook_preprocess_HOOK().
 */
function gin_page_attachments_alter(array &$attachments): void {
}
"#,
        conventions,
        expect![[r#"
            6:10-6:36 warning [misnamed-implementation] gin_page_attachments_alter() is documented as implementing hook_preprocess_HOOK() and is not named for it, so nothing will call it
        "#]],
    );
}

#[test]
fn every_shape_of_placeholder_a_real_hook_carries_is_left_alone() {
    // Each of these is a correct implementation whose name does not contain the hook's. The
    // uppercase run is one hole however many underscores its author wrote inside it, which is what
    // `ENTITY_TYPE` needs and `FORM_ID` needs and a per-word rule gets wrong.
    check_framework(
        r#"
//- /modules/core/core.api.php
<?php
function hook_form_FORM_ID_alter(array &$form): void {
}
function hook_ENTITY_TYPE_delete(object $entity): void {
}
function hook_preprocess_HOOK(array &$variables): void {
}
function hook_form_alter(array &$form): void {
}

//- /modules/app_core/app_core.php
<?php

/**
 * Implements hook_form_FORM_ID_alter().
 */
function app_core_form_views_exposed_form_alter(array &$form): void {
}

/**
 * Implements hook_ENTITY_TYPE_delete().
 */
function app_core_user_delete(object $entity): void {
}

/**
 * Implements hook_preprocess_HOOK().
 */
function app_core_preprocess_menu__toolbar__gin(array &$variables): void {
}

/**
 * Implements hook_form_alter().
 */
function app_core_form_alter(array &$form): void {
}
"#,
        conventions,
        expect![[r#""#]],
    );
}

#[test]
fn a_claim_is_checked_even_where_nothing_declares_the_hook() {
    // The claim fixes the name whether or not the hook is one this checkout holds -- a project's own
    // hooks are invoked by name and declared nowhere, and the name still has to agree.
    check_framework(
        r#"
//- /modules/wh_order_reports/wh_order_reports.php
<?php

/**
 * Implements hook_app_core_thing_happened().
 */
function wh_order_reports_app_core_thing_happened(): void {
}

/**
 * Implements hook_app_core_thing_happened().
 */
function wh_order_reports_something_else(): void {
}
"#,
        conventions,
        expect![[r#"
            6:10-6:50 warning [undeclared-implementation] nothing declares or dispatches hook_app_core_thing_happened(), so wh_order_reports_app_core_thing_happened() implements nothing and never runs
            12:10-12:41 warning [misnamed-implementation] wh_order_reports_something_else() is documented as implementing hook_app_core_thing_happened() and is not named for it, so nothing will call it
            12:10-12:41 warning [undeclared-implementation] nothing declares or dispatches hook_app_core_thing_happened(), so wh_order_reports_something_else() implements nothing and never runs
        "#]],
    );
}

#[test]
fn a_method_is_not_reached_by_its_own_name_so_nothing_is_checked() {
    // A framework that dispatches to a method does it through an attribute or a registry, neither of
    // which this could read off the name. Reporting one would report every object-oriented hook.
    check_framework(
        r#"
//- /modules/app_core/app_core.php
<?php
class FormHooks
{
    /**
     * Implements hook_form_alter().
     */
    public function alterAnyForm(array &$form): void
    {
    }
}
"#,
        conventions,
        expect![[r#""#]],
    );
}

#[test]
fn a_sentence_that_is_not_the_opening_of_the_docblock_is_prose() {
    // A docblock may mention another hook in passing. Only the opening sentence is the claim, and
    // reading a mention as one would report a function for describing its neighbours.
    check_framework(
        r#"
//- /modules/app_core/app_core.php
<?php

/**
 * Rebuilds the report cache.
 *
 * Runs after hook_form_alter() has settled the form.
 */
function app_core_rebuild_reports(): void {
}
"#,
        conventions,
        expect![[r#""#]],
    );
}

#[test]
fn nothing_is_reported_where_no_framework_dispatches_by_name() {
    // The whole rule arrives from the seam. A project with no framework has no claim to check, and
    // the same file must be silent.
    check(
        r#"
//- /modules/app_region_notice/app_region_notice.php
<?php

/**
 * Implements hook_notifications_alter().
 */
function app_region_notifications_alter(): array {
    return [];
}
"#,
        expect![[r#""#]],
    );
}
