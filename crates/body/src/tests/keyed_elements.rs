//! Array entries written with a key a literal spells out.
//!
//! The one shape in PHP where a string means something and no call says so. A framework's render
//! array is nothing but these -- `['#type' => 'textfield']` -- and until this existed the only way
//! into a catalog was through a receiver and a method name, so an array literal was invisible to
//! every reader that resolves a string.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner, LineIndex, TextSize};

use crate::{keyed_element_at, keyed_elements};

/// Every keyed entry in the file, as `key => value-text` with where the value sits.
///
/// The whole file and not a sample: what this projection is for is a report that sweeps a file, and
/// an entry it invents has to be as visible as one it drops.
fn check_all(source: &str, expect: Expect) {
    let (hir, text) = lower(source);
    let lines = LineIndex::new(&text);

    let mut out = String::new();
    for element in keyed_elements(&hir) {
        let start = lines.line_col(element.value.start());
        let written = &text[usize::from(element.value.start())..usize::from(element.value.end())];
        out.push_str(&format!(
            "{} => {written} @ {}:{}\n",
            element.key.as_str(),
            start.line + 1,
            start.col + 1
        ));
    }

    expect.assert_eq(&out);
}

/// The key of the entry whose value holds `$0`.
fn check_at(source: &str, expect: Expect) {
    let offset = TextSize::try_from(source.find("$0").expect("the source marks a position"))
        .expect("fixture fits in u32");
    let (hir, _) = lower(&source.replace("$0", ""));

    expect.assert_eq(&match keyed_element_at(&hir, offset) {
        Some(element) => element.key.as_str().to_owned(),
        None => "<no keyed element>".to_owned(),
    });
}

fn lower(source: &str) -> (phrost_hir::FileHir, String) {
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        source,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );
    (lowered.hir, source.to_owned())
}

#[test]
fn every_literally_keyed_entry_is_found_and_nothing_else_is() {
    check_all(
        r#"<?php
function build(string $name, array $rest): array {
    return [
        '#type' => 'textfield',
        '#title' => $name,
        0 => 'first',
        $name => 'computed',
        'bare',
        ...$rest,
    ];
}
"#,
        // A computed key names no slot this layer can spell, a spread names none at all, and a bare
        // element is keyed by a position nothing here counts -- so none of the three is an entry.
        // An integer key is one: PHP really keys by it, and it is written out.
        expect![[r#"
            #type => 'textfield' @ 4:20
            #title => $name @ 5:21
            0 => 'first' @ 6:14
        "#]],
    );
}

#[test]
fn a_nested_array_contributes_its_own_entries() {
    check_all(
        r#"<?php
function build(): array {
    return [
        '#attached' => ['library' => ['mine/thing']],
    ];
}
"#,
        expect![[r#"
            #attached => ['library' => ['mine/thing']] @ 4:24
            library => 'mine/thing' @ 4:39
        "#]],
    );
}

#[test]
fn a_key_whose_value_is_a_flat_list_of_strings_names_every_item() {
    // `'library' => ['a/b', 'c/d']` attaches two libraries, not one array -- so the key repeats,
    // once per item, and the value narrows to the item alone rather than the whole list.
    check_all(
        r#"<?php
function build(): array {
    return [
        '#attached' => ['library' => ['a/b', 'c/d']],
    ];
}
"#,
        expect![[r#"
            #attached => ['library' => ['a/b', 'c/d']] @ 4:24
            library => 'a/b' @ 4:39
            library => 'c/d' @ 4:46
        "#]],
    );
}

#[test]
fn a_list_holding_a_computed_entry_still_reports_the_whole_array() {
    // Nothing in the list is a bare string this layer can spell, so the key still names the array
    // itself -- the same rule `'#attached'` already relies on for its own, keyed, elements.
    check_all(
        r#"<?php
function build(string $name): array {
    return [
        'library' => [$name],
    ];
}
"#,
        expect![[r#"
            library => [$name] @ 4:22
        "#]],
    );
}

#[test]
fn a_cursor_on_one_item_of_a_list_finds_the_key_that_governs_it() {
    check_at(
        r#"<?php
function build(): array {
    return ['#attached' => ['library' => ['a/b', 'c$0/d']]];
}
"#,
        expect!["library"],
    );
}

#[test]
fn a_cursor_in_a_value_finds_the_entry_that_holds_it() {
    check_at(
        r#"<?php
function build(): array {
    return ['#type' => 'text$0field'];
}
"#,
        expect!["#type"],
    );
}

#[test]
fn a_cursor_in_a_key_finds_nothing() {
    // The key is what says what the value means, so a cursor writing the key is not writing a
    // value -- and answering with what the key means would offer element ids where property names
    // belong.
    check_at(
        r#"<?php
function build(): array {
    return ['#ty$0pe' => 'textfield'];
}
"#,
        expect!["<no keyed element>"],
    );
}

#[test]
fn the_innermost_entry_wins() {
    // Three deep, and only the nearest key says what the string being written is.
    check_at(
        r#"<?php
function build(): array {
    return ['#attached' => ['library' => ['mine/th$0ing']]];
}
"#,
        expect!["library"],
    );
}

#[test]
fn an_entry_whose_key_is_computed_leaves_the_one_around_it_answering() {
    // The inner entry is nearer and this layer cannot spell its key, so taking it and then dropping
    // it would lose the outer entry that does answer.
    check_at(
        r#"<?php
function build(string $name): array {
    return ['#attached' => [$name => 'mine/th$0ing']];
}
"#,
        expect!["#attached"],
    );
}
