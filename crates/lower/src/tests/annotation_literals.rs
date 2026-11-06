//! Which of a docblock annotation's string literals reach the HIR.
//!
//! The older spelling of an attribute, and it has to arrive as the same thing: a project carrying
//! both -- which every Drupal checkout of any age does -- would otherwise have two catalogs of one
//! kind of id, filled by two rules with two invalidation stories.
//!
//! Only for a name that was handed down, and only where mago says a tag is. Both gates are here,
//! because both are silent when they are wrong: an ungated reader parses every docblock in the
//! project, and one that reads its own text rather than mago's tags reads the worked example in
//! Drupal's own `core.api.php` as a declaration.

use expect_test::{Expect, expect};
use phrost_hir::AttributeArgument;
use phrost_model::{FileId, Interner};

use crate::KeyedAttributes;

/// Every attribute literal the file's class-likes carry, one per line.
///
/// The whole set and not a lookup, for the reason the attribute half gives: what this is about is
/// which literals were *kept*, so an extra one has to be as visible as a missing one.
fn check(source: &str, wanted: &[&str], expect: Expect) {
    let mut interner = Interner::new();
    let lowered = crate::lower_text(
        &mut interner,
        FileId::from_raw(0),
        source,
        &KeyedAttributes::of(wanted.iter().copied()),
    );
    let hir = &lowered.hir;

    let mut out = String::new();
    for (_, class) in hir.classes() {
        for id in hir.attribute_literal_list(class.attribute_literals) {
            let literal = hir.attribute_literal(*id);
            let slot = match &literal.argument {
                AttributeArgument::Named(name) => name.as_str().to_owned(),
                AttributeArgument::At(position) => format!("#{position}"),
            };
            out.push_str(&format!(
                "{} {} {} = {}\n",
                class.name.as_str(),
                literal.attribute.as_str(),
                slot,
                literal.value
            ));
        }
    }

    expect.assert_eq(&out);
}

#[test]
fn an_annotation_declares_what_the_attribute_would_have() {
    check(
        r#"<?php
/**
 * Defines the Behavior settings entity.
 *
 * @ConfigEntityType(
 *   id = "behavior_settings",
 *   label = @Translation("Rabbit hole settings"),
 *   entity_keys = {
 *     "id" = "id",
 *     "uuid" = "uuid"
 *   },
 *   config_prefix = "behavior_settings"
 * )
 */
class BehaviorSettings {}
"#,
        &["ConfigEntityType"],
        // `label` is a nested annotation and carries no literal; the `entity_keys` map is values
        // rather than arguments, which is what stops every entity type claiming the id `id`.
        expect![[r#"
            BehaviorSettings ConfigEntityType id = behavior_settings
            BehaviorSettings ConfigEntityType config_prefix = behavior_settings
        "#]],
    );
}

#[test]
fn nothing_is_read_from_a_docblock_for_an_attribute_nobody_named() {
    // The default for a project no framework was detected in. Reading every `@Something(...)` in
    // every docblock would parse the whole project's documentation to collect strings nothing
    // consults.
    check(
        r#"<?php
/**
 * @ConfigEntityType(
 *   id = "behavior_settings"
 * )
 */
class BehaviorSettings {}
"#,
        &[],
        expect![""],
    );
}

#[test]
fn a_shorthand_annotation_writes_its_id_positionally() {
    // `@RenderElement("horizontal_tabs")` is how a single-key annotation is written, and it has to
    // land at position zero -- the same slot `#[RenderElement('horizontal_tabs')]` lands in, since
    // one rule has to match both spellings.
    check(
        r#"<?php
/**
 * Provides a horizontal tabs element.
 *
 * @RenderElement("horizontal_tabs")
 */
class HorizontalTabs {}
"#,
        &["RenderElement"],
        expect![[r#"
            HorizontalTabs RenderElement #0 = horizontal_tabs
        "#]],
    );
}

#[test]
fn a_worked_example_inside_a_code_block_declares_nothing() {
    // Drupal's `core.api.php` documents the annotation syntax exactly like this, with a real entity
    // type id in it. Reading it would put a second claim on `comment` from a file that declares no
    // class, and a key two declarations claim answers nothing at all -- so the id would stop
    // resolving rather than start.
    check(
        r#"<?php
/**
 * Annotations for class discovery.
 *
 * To annotate a class as a plugin, write:
 * @code
 * * @ContentEntityType(
 * *   id = "comment",
 * *   base_table = "comment"
 * * )
 * @endcode
 */
class AnnotationDocumentation {}
"#,
        &["ContentEntityType"],
        expect![""],
    );
}

#[test]
fn both_spellings_on_one_class_are_both_read() {
    // What a module carrying an attribute for Drupal 10.2 and an annotation for everything older
    // actually writes. Both arrive, and the index deduplicates the claim: two spellings of one
    // declaration are one claim, not two classes fighting over a key.
    check(
        r#"<?php
/**
 * Defines the paragraph entity.
 *
 * @ContentEntityType(
 *   id = "paragraph"
 * )
 */
#[ContentEntityType(id: 'paragraph')]
class Paragraph {}
"#,
        &["ContentEntityType"],
        expect![[r#"
            Paragraph ContentEntityType id = paragraph
            Paragraph ContentEntityType id = paragraph
        "#]],
    );
}

#[test]
fn an_annotation_on_something_that_is_not_a_class_is_not_read() {
    // Only a class-like declaration claims a catalog key, because the class is what the key names.
    // A function's docblock has nothing for an id to point at.
    check(
        r#"<?php
/**
 * @ContentEntityType(
 *   id = "not_a_class"
 * )
 */
function make_something() {}
"#,
        &["ContentEntityType"],
        expect![""],
    );
}
