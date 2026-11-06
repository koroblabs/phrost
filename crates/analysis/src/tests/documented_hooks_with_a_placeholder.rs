//! A docblock naming a declaration whose own name has a variable part.
//!
//! The other half of the placeholder rule. `misnamed-implementation` reads a *reference* carrying
//! holes against a concrete declaration -- can dispatch reach this function from this sentence --
//! and this reads the roles swapped: the sentence is concrete and the declaration is the one
//! spelled with a hole. `Implements hook_node_delete()` is documented on `hook_ENTITY_TYPE_delete`,
//! and the two are one declaration under the convention's own naming rule rather than a near miss.
//!
//! Seven of the eleven hooks one real checkout failed to follow were this, so what is under test is
//! the shape of every one of them: a leading hole, a hole in the middle, a trailing hole, and a
//! hole made of two capitalised words.

use expect_test::expect;
use phrost_harness::World;

use super::{Query, check_with};

/// The hooks whose declared names carry a placeholder, as Drupal writes them.
const CORE: &str = r#"
//- /core/entity.api.php
<?php

/**
 * Respond to entity deletion.
 *
 * @param \Drupal\Core\Entity\EntityInterface $entity
 *   The entity object.
 */
function hook_ENTITY_TYPE_delete(EntityInterface $entity) {
}

/**
 * Control access to an entity.
 */
function hook_ENTITY_TYPE_access(EntityInterface $entity, $operation) {
}

/**
 * Respond to the deletion of a translation.
 */
function hook_ENTITY_TYPE_translation_delete(EntityInterface $translation) {
}

//- /core/form.api.php
<?php

/**
 * Provide a form-specific alteration.
 */
function hook_form_FORM_ID_alter(array &$form, string $form_id) {
}

/**
 * Provide a base-form-specific alteration.
 */
function hook_form_BASE_FORM_ID_alter(array &$form, string $form_id) {
}

//- /core/theme.api.php
<?php

/**
 * Preprocess theme variables for a specific theme hook.
 */
function hook_preprocess_HOOK(array &$variables) {
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
fn a_written_name_reaches_the_declaration_whose_hole_it_fills() {
    check_with(
        &format!(
            r#"{CORE}
//- /modules/app_core/app_core.php
<?php

/**
 * Implements hook_no@leading@de_delete().
 */
function app_core_node_delete(NodeInterface $node): void {{
}}

/**
 * Implements hook_form_views_expo@middle@sed_form_alter().
 */
function app_core_form_views_exposed_form_alter(array &$form): void {{
}}

/**
 * Implements hook_prepro@trailing@cess_menu().
 */
function app_core_preprocess_menu(array &$variables): void {{
}}
"#
        ),
        &[
            Query::goto("a hole where the name opens", "leading"),
            Query::goto("a hole in the middle of the name", "middle"),
            Query::goto("a hole where the name ends", "trailing"),
        ],
        conventions,
        expect![[r#"
            a hole where the name opens
            - function hook_ENTITY_TYPE_delete @ core/entity.api.php 9:10-9:33 (full 9:1-10:2)
            a hole in the middle of the name
            - function hook_form_FORM_ID_alter @ core/form.api.php 6:10-6:33 (full 6:1-7:2)
            a hole where the name ends
            - function hook_preprocess_HOOK @ core/theme.api.php 6:10-6:30 (full 6:1-7:2)
        "#]],
    );
}

#[test]
fn the_hover_credits_the_declaration_and_not_the_sentence() {
    // The words belong to `hook_ENTITY_TYPE_delete`; a note naming the sentence would attribute
    // them to a declaration that has none.
    check_with(
        &format!(
            r#"{CORE}
//- /modules/app_core/app_core.php
<?php

/**
 * Implements hook_node_delete().
 */
function app_core_$0node_delete(NodeInterface $node): void {{
}}
"#
        ),
        &[Query::hover("hover on the implementation", "0")],
        conventions,
        expect![[r#"
            hover on the implementation
            - code: <?php
            function app_core_node_delete(NodeInterface $node): void
            - ---
            - doc: Implements hook_node_delete().
            - ---
            - note: documentation below is from hook_ENTITY_TYPE_delete()
            - doc: Respond to entity deletion.
            - tag: Parameter $entity \Drupal\Core\Entity\EntityInterface The entity object.
        "#]],
    );
}

#[test]
fn a_hole_stands_for_at_least_one_part_and_the_words_around_it_still_have_to_be_there() {
    // `hook_delete` is not `hook_ENTITY_TYPE_delete` with nothing in the hole, and
    // `hook_node_removed` shares the hole's neighbours with nothing.
    check_with(
        &format!(
            r#"{CORE}
//- /modules/app_core/app_core.php
<?php

/**
 * Implements hook_de@empty@lete().
 */
function app_core_delete(): void {{
}}

/**
 * Implements hook_node_rem@elsewhere@oved().
 */
function app_core_node_removed(): void {{
}}
"#
        ),
        &[
            Query::goto("a hole filled with nothing", "empty"),
            Query::goto("a name the words around the hole rule out", "elsewhere"),
        ],
        conventions,
        expect![[r#"
            a hole filled with nothing
            - (nothing to go to)
            a name the words around the hole rule out
            - (nothing to go to)
        "#]],
    );
}

#[test]
fn the_declaration_that_spells_more_of_the_name_wins() {
    // `hook_ENTITY_TYPE_delete` and `hook_ENTITY_TYPE_translation_delete` both open with a hole,
    // and only one of them says anything about the part before `delete`.
    check_with(
        &format!(
            r#"{CORE}
//- /modules/app_core/app_core.php
<?php

/**
 * Implements hook_node_translat@specific@ion_delete().
 */
function app_core_node_translation_delete(): void {{
}}
"#
        ),
        &[Query::goto("two holes, one of them narrower", "specific")],
        conventions,
        expect![[r#"
            two holes, one of them narrower
            - function hook_ENTITY_TYPE_translation_delete @ core/entity.api.php 21:10-21:45 (full 21:1-22:2)
        "#]],
    );
}

#[test]
fn between_two_declarations_that_say_as_much_the_less_qualified_placeholder_wins() {
    // `hook_form_FORM_ID_alter` and `hook_form_BASE_FORM_ID_alter` describe the same names, because
    // whether a form id is a base form id is a fact about the form. The general one is where a
    // reader with nothing else to go on is better sent.
    check_with(
        &format!(
            r#"{CORE}
//- /modules/app_core/app_core.php
<?php

/**
 * Implements hook_form_user_regi@ambiguous@ster_form_alter().
 */
function app_core_form_user_register_form_alter(array &$form): void {{
}}
"#
        ),
        &[Query::goto(
            "two placeholders describing one set",
            "ambiguous",
        )],
        conventions,
        expect![[r#"
            two placeholders describing one set
            - function hook_form_FORM_ID_alter @ core/form.api.php 6:10-6:33 (full 6:1-7:2)
        "#]],
    );
}

#[test]
fn without_the_convention_a_placeholder_is_only_a_name_written_oddly() {
    // The whole rule arrives from the seam. A project that states no convention has no reason to
    // read an uppercase run as anything, so the sentence stays prose.
    super::check(
        &format!(
            r#"{CORE}
//- /modules/app_core/app_core.php
<?php

/**
 * Implements hook_no@leading@de_delete().
 */
function app_core_node_delete(NodeInterface $node): void {{
}}
"#
        ),
        &[Query::goto("a hole where the name opens", "leading")],
        expect![[r#"
            a hole where the name opens
            - (nothing to go to)
        "#]],
    );
}
