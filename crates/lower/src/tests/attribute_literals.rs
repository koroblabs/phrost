//! Which of an attribute's string literals reach the HIR.
//!
//! Named ones always, because a project writes about a thousand of them and deciding costs more than
//! keeping. Positional ones only for the attributes the caller asked for, because the same project
//! writes 8 402 of those and 77 mean anything -- so the list is handed down and this is where the
//! gate it opens is checked.
//!
//! And the *entries* of an array argument only for the arguments the caller named, which is the
//! strictest gate of the three: entering one keeps a string per element rather than one per argument,
//! and an entity type's `handlers` beside its `links` is a dozen class names nothing asks about.

use expect_test::{Expect, expect};
use phrost_hir::AttributeArgument;
use phrost_model::{FileId, Interner};

use crate::KeyedAttributes;

/// Every attribute literal the file's class-likes carry, one per line.
///
/// The whole set and not a lookup: what this test is about is which literals were *kept*, so an
/// extra one has to be as visible as a missing one.
fn check(source: &str, wanted: &[&str], expect: Expect) {
    check_entering(source, wanted, &[], expect);
}

/// The same, also entering the array arguments named.
fn check_entering(source: &str, wanted: &[&str], entered: &[&str], expect: Expect) {
    let mut interner = Interner::new();
    let lowered = crate::lower_text(
        &mut interner,
        FileId::from_raw(0),
        source,
        &KeyedAttributes::of(wanted.iter().copied()).entering(entered.iter().copied()),
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
            let slot = match &literal.key {
                Some(key) => format!("{slot}[{key}]"),
                None => slot,
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
fn a_named_literal_is_kept_whether_or_not_anybody_asked_for_the_attribute() {
    check(
        r#"<?php
#[ContentEntityType(id: 'node', label: 'Content')]
class Node {}
"#,
        &[],
        expect![[r#"
            Node ContentEntityType id = node
            Node ContentEntityType label = Content
        "#]],
    );
}

#[test]
fn a_positional_literal_is_kept_only_for_an_attribute_that_was_asked_for() {
    check(
        r#"<?php
#[FormElement('textfield')]
class Textfield {}

#[Group('functional')]
class SomeTest {}
"#,
        &["FormElement"],
        expect![[r#"
            Textfield FormElement #0 = textfield
        "#]],
    );
}

#[test]
fn nothing_positional_is_kept_when_nothing_was_asked_for() {
    // The default for a project no framework was detected in, and the shape the measurement is
    // about: 8 402 of these exist in one real checkout and none of them would be worth a byte.
    check(
        r#"<?php
#[FormElement('textfield')]
class Textfield {}
"#,
        &[],
        expect![""],
    );
}

#[test]
fn a_qualified_attribute_is_the_same_attribute() {
    // Both spellings are in real code, and an attribute is resolved by its last segment.
    check(
        r#"<?php
#[\Drupal\Core\Render\Attribute\RenderElement('page')]
class Page {}
"#,
        &["RenderElement"],
        // Stored as written, which is what the reader compares by its last segment too -- resolving
        // it here would mean running the def-map over every attribute in the project during ingest.
        expect![[r#"
            Page Drupal\Core\Render\Attribute\RenderElement #0 = page
        "#]],
    );
}

#[test]
fn a_position_counts_over_the_whole_list_and_a_spread_writes_none() {
    // The number has to mean what a reader of the source would count, or a contract naming
    // argument 1 reaches argument 2. A spread fills however many are left, so it is no position at
    // all -- and an argument that is not a literal has no value to record, which leaves a gap in
    // the numbering rather than closing it.
    check(
        r#"<?php
#[Plugin('first', SomeClass::class, 'third', ...$rest)]
class Widget {}
"#,
        &["Plugin"],
        expect![[r#"
            Widget Plugin #0 = first
            Widget Plugin #2 = third
        "#]],
    );
}

#[test]
fn the_two_spellings_do_not_answer_for_each_other() {
    // A named literal is `Named`, and stays so even for an attribute whose positional literals were
    // asked for. A contract on `id:` must not match a bare first argument, or every attribute with a
    // leading string would claim keys nobody declared.
    check(
        r#"<?php
#[Block(id: 'mine', admin_label: 'Mine')]
class MyBlock {}
"#,
        &["Block"],
        expect![[r#"
            MyBlock Block id = mine
            MyBlock Block admin_label = Mine
        "#]],
    );
}

#[test]
fn an_array_argument_keeps_its_entries_only_where_a_rule_asked() {
    // `handlers` is the one to watch: it sits beside `links` in every entity type Drupal ships and is
    // a dozen class names, so entering both would be the whole cost of this mechanism spent on
    // nothing. The value of an entry travels with its key because it is the useful half -- a route's
    // path is what hovering the generated name shows.
    check_entering(
        r#"<?php
#[ContentEntityType(
    id: 'node',
    links: [
        'canonical' => '/node/{node}',
        'edit-form' => '/node/{node}/edit',
        'drupal:content-translation-overview' => '/node/{node}/translations',
    ],
    handlers: [
        'storage' => 'Drupal\node\NodeStorage',
    ],
)]
class Node {}
"#,
        &["ContentEntityType"],
        &["links"],
        expect![[r#"
            Node ContentEntityType id = node
            Node ContentEntityType links[canonical] = /node/{node}
            Node ContentEntityType links[edit-form] = /node/{node}/edit
            Node ContentEntityType links[drupal:content-translation-overview] = /node/{node}/translations
        "#]],
    );
}

#[test]
fn an_array_argument_of_an_attribute_nobody_asked_about_is_left_alone() {
    // The `id` line is what makes this a test rather than an empty snapshot: a named string literal
    // is kept for every attribute, asked about or not, so the file *does* reach the HIR -- and the
    // entries of its array are still left where they were written.
    check_entering(
        r#"<?php
#[Group(id: 'reviewers', links: ['canonical' => '/group'])]
class Group {}
"#,
        &["ContentEntityType"],
        &["links"],
        expect![[r#"
            Group Group id = reviewers
        "#]],
    );
}

#[test]
fn the_older_spelling_of_an_array_argument_is_entered_too() {
    // The annotation walk steps over every `{ }` group on purpose -- `entity_keys = { "id" = "id" }`
    // would otherwise claim the id `id` for every entity type in the project -- so entering one is a
    // deliberate exception for the argument a rule named, and everything else stays stepped over.
    check_entering(
        r#"<?php
/**
 * @ContentEntityType(
 *   id = "block_content",
 *   entity_keys = {
 *     "id" = "id",
 *   },
 *   links = {
 *     "canonical" = "/block/{block_content}",
 *     "delete-form" = "/block/{block_content}/delete",
 *   },
 * )
 */
class BlockContent {}
"#,
        &["ContentEntityType"],
        &["links"],
        expect![[r#"
            BlockContent ContentEntityType id = block_content
            BlockContent ContentEntityType links[canonical] = /block/{block_content}
            BlockContent ContentEntityType links[delete-form] = /block/{block_content}/delete
        "#]],
    );
}
