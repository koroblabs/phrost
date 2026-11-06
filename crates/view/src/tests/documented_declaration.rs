//! Which declaration a name written in a docblock names, when the declaration's name has a hole.
//!
//! The projection that owns the rule, asked directly rather than through goto: every written name
//! against every declaration, so what the rule accepts and what it refuses are one table instead of
//! a query per case. The reference is concrete here and the *declaration* carries the placeholder,
//! which is `dispatch_reaches` with the roles swapped -- and the same matcher, because two matchers
//! for one naming rule is how they come to disagree.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};

use crate::IndexView;

/// Every declaration the project holds, and what each written name resolves to.
fn check(fixture: &str, written: &[&str], expect: Expect) {
    let mut world = World::parse(fixture);
    world.contracts_mut().add_documented_elsewhere(
        "Implements",
        "hook_",
        Some("extension"),
        Some("hook"),
    );
    let world = world;
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        written
            .iter()
            .map(|name| match view.documented_declaration(name) {
                Some(declared) => format!("{name} -> {declared}\n"),
                None => format!("{name} -> nothing\n"),
            })
            .collect::<String>()
    });

    expect.assert_eq(&rendered);
}

/// Every placeholder shape a real checkout's hooks are spelled in, in one file nothing else reads.
const HOOKS: &str = r#"
//- /app.php
<?php
function hook_ENTITY_TYPE_access() {}
function hook_ENTITY_TYPE_delete() {}
function hook_ENTITY_TYPE_translation_delete() {}
function hook_form_BASE_FORM_ID_alter() {}
function hook_form_FORM_ID_alter() {}
function hook_preprocess_HOOK() {}
function hook_theme_suggestions_HOOK_alter() {}
function hook_cron() {}
function hook_theme() {}
"#;

#[test]
fn a_written_name_resolves_to_the_declaration_whose_holes_it_fills() {
    check(
        HOOKS,
        &[
            // A name spelled the way its declaration spells it, which needs no rule at all.
            "hook_cron",
            "hook_theme",
            // A hole where the name opens, in the middle, and where it ends.
            "hook_node_delete",
            "hook_user_delete",
            "hook_file_access",
            "hook_form_views_exposed_form_alter",
            "hook_preprocess_menu",
            // A hole standing for more than one part.
            "hook_preprocess_file_link",
            // Two declarations open with a hole and one says more about what follows it.
            "hook_node_translation_delete",
            // Two say exactly as much, so the less qualified placeholder answers.
            "hook_form_user_register_form_alter",
        ],
        expect![[r#"
            hook_cron -> hook_cron
            hook_theme -> hook_theme
            hook_node_delete -> hook_ENTITY_TYPE_delete
            hook_user_delete -> hook_ENTITY_TYPE_delete
            hook_file_access -> hook_ENTITY_TYPE_access
            hook_form_views_exposed_form_alter -> hook_form_FORM_ID_alter
            hook_preprocess_menu -> hook_preprocess_HOOK
            hook_preprocess_file_link -> hook_preprocess_HOOK
            hook_node_translation_delete -> hook_ENTITY_TYPE_translation_delete
            hook_form_user_register_form_alter -> hook_form_FORM_ID_alter
        "#]],
    );
}

#[test]
fn a_name_no_declaration_describes_resolves_to_nothing() {
    check(
        HOOKS,
        &[
            // Removed in a major version. There is nowhere to go, and a hole must not be stretched
            // to cover the difference.
            "hook_field_widget_form_alter",
            // A hook the project invokes itself and documents nowhere.
            "hook_notifications_alter",
            "hook_webhook_event_alter",
            "hook_app_core_order_reports_requested",
            // A hole stands for at least one part, so neither of these is `hook_preprocess_HOOK`.
            "hook_preprocess",
            "hook_delete",
            // The words around the hole still have to be written.
            "hook_node_removed",
            // The prefix alone names nothing, and neither does a word that merely starts with it.
            "hook_",
            "hooks_are_not_hooks",
        ],
        expect![[r#"
            hook_field_widget_form_alter -> nothing
            hook_notifications_alter -> nothing
            hook_webhook_event_alter -> nothing
            hook_app_core_order_reports_requested -> nothing
            hook_preprocess -> nothing
            hook_delete -> nothing
            hook_node_removed -> nothing
            hook_ -> nothing
            hooks_are_not_hooks -> nothing
        "#]],
    );
}

#[test]
fn a_class_is_not_a_declaration_a_docblock_convention_names() {
    // The short-name index the range runs over holds every kind of declaration, and dispatch by
    // name binds a function. A class called `hook_ENTITY_TYPE_thing` must not answer for one.
    check(
        r#"
//- /app.php
<?php
class hook_ENTITY_TYPE_delete {}
"#,
        &["hook_node_delete"],
        expect![[r#"
            hook_node_delete -> nothing
        "#]],
    );
}

#[test]
fn without_a_convention_a_placeholder_is_nothing_but_a_name_spelled_loudly() {
    // The rule arrives from the seam, and only the rule: a project with no framework pays one
    // emptiness check and reads no uppercase run as standing for anything, while a name spelled the
    // way its declaration spells it needs no convention to be found.
    let world = World::parse(HOOKS);
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        format!(
            "hook_node_delete -> {:?}\nhook_cron -> {:?}\n",
            view.documented_declaration("hook_node_delete")
                .map(|name| name.as_str().to_owned()),
            view.documented_declaration("hook_cron")
                .map(|name| name.as_str().to_owned()),
        )
    });

    expect![[r#"
        hook_node_delete -> None
        hook_cron -> Some("hook_cron")
    "#]]
    .assert_eq(&rendered);
}
