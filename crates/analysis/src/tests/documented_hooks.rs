//! A docblock that names the declaration carrying the documentation.
//!
//! One line -- `Implements hook_form_alter().` -- is the whole of what a Drupal hook implementation
//! is documented with, and the twenty lines that say what the parameters mean are written once, on
//! the hook. The index already holds both, so the questions here are whether hover follows the
//! sentence and whether the name inside it is somewhere a reader can go.

use expect_test::expect;
use phrost_harness::World;

use super::{Query, check, check_with};

/// The documentation is on the hook, the implementation is somewhere else entirely.
const PROJECT: &str = r#"
//- /core/form.api.php
<?php

/**
 * Perform alterations before a form is rendered.
 *
 * One of the most commonly implemented hooks.
 *
 * @param array $form
 *   Nested array of form elements that comprise the form.
 * @param string $form_id
 *   String representing the name of the form itself.
 */
function hook_form_alter(array &$form, string $form_id) {
}

//- /modules/app_core/app_core.php
<?php

/**
 * Implements hook_form_al@reference@ter().
 */
function app_core_form_$0alter(array &$form, string $form_id): void {
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
        .catalog_entry("extension", "app_core", None);
}

#[test]
fn hover_on_an_implementation_shows_what_the_hook_says() {
    check_with(
        PROJECT,
        &[
            Query::hover("hover on the implementation", "0"),
            Query::goto("goto from the sentence", "reference"),
            Query::hover("hover on the sentence", "reference"),
        ],
        conventions,
        expect![[r#"
            hover on the implementation
            - code: <?php
            function app_core_form_alter(array &$form, string $form_id): void
            - ---
            - doc: Implements hook_form_alter().
            - ---
            - note: documentation below is from hook_form_alter()
            - doc: Perform alterations before a form is rendered.

            One of the most commonly implemented hooks.
            - tag: Parameter $form array Nested array of form elements that comprise the form.
            - tag: Parameter $form_id string String representing the name of the form itself.
            goto from the sentence
            - function hook_form_alter @ core/form.api.php 13:10-13:25 (full 13:1-14:2)
            hover on the sentence
            - code: <?php
            function hook_form_alter(array &$form, string $form_id)
            - ---
            - doc: Perform alterations before a form is rendered.

            One of the most commonly implemented hooks.
            - tag: Parameter $form array Nested array of form elements that comprise the form.
            - tag: Parameter $form_id string String representing the name of the form itself.
        "#]],
    );
}

#[test]
fn without_the_convention_the_sentence_is_prose_and_nothing_follows_it() {
    // The whole feature arrives from the seam: a project with no framework must see a comment.
    check(
        PROJECT,
        &[
            Query::hover("hover on the implementation", "0"),
            Query::goto("goto from the sentence", "reference"),
        ],
        expect![[r#"
            hover on the implementation
            - code: <?php
            function app_core_form_alter(array &$form, string $form_id): void
            - ---
            - doc: Implements hook_form_alter().
            goto from the sentence
            - (nothing to go to)
        "#]],
    );
}

#[test]
fn a_name_the_convention_marks_anywhere_in_a_comment_is_a_target() {
    // `@see hook_theme()` is written constantly in Drupal, and it is the same fact as the claim: a
    // declaration named where nothing else would look for one.
    check_with(
        r#"
//- /core/theme.api.php
<?php

/**
 * Registers theme implementations.
 */
function hook_theme(array $existing) {
}

//- /modules/app_core/app_core.php
<?php

/**
 * Builds the report table.
 *
 * @see hook_th@see@eme()
 */
function app_core_report_table(): array {
    return [];
}
"#,
        &[Query::goto("goto from a see tag", "see")],
        conventions,
        expect![[r#"
            goto from a see tag
            - function hook_theme @ core/theme.api.php 6:10-6:20 (full 6:1-7:2)
        "#]],
    );
}

#[test]
fn a_sentence_naming_a_hook_nothing_declares_answers_nothing() {
    // A deterministic miss. The docblock can name a hook removed in a major version, and there is
    // nowhere to go and nothing to say -- which is a different answer from guessing at a near miss.
    check_with(
        r#"
//- /modules/app_core/app_core.php
<?php

/**
 * Implements hook_field_widget_f@gone@orm_alter().
 */
function app_core_field_widget_form_alter(array &$element): void {
}
"#,
        &[
            Query::goto("goto from the sentence", "gone"),
            Query::hover("hover on the sentence", "gone"),
        ],
        conventions,
        expect![[r#"
            goto from the sentence
            - (nothing to go to)
            hover on the sentence
            - (nothing to describe)
        "#]],
    );
}
