//! Which `[...]` in a file is a *read* of an element.
//!
//! What the layer above resolves a type for, so what it leaves out is the whole of what it can never
//! report: a write creates the array it writes into, and a presence test is asked precisely because
//! the slot may not be there.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner};

use crate::subscript_sites;

/// Every element read the source makes, as the base name and the key it asked for.
fn check(source: &str, expect: Expect) {
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

    let text = &lowered.hir;
    let mut out = String::new();
    for site in subscript_sites(text) {
        out.push_str(&format!(
            "{:?} key {}\n",
            &source[usize::from(site.range.start())..usize::from(site.range.end())],
            site.key.as_ref().map_or("-", |key| key.as_str()),
        ));
    }
    expect.assert_eq(&out);
}

#[test]
fn a_read_of_an_element_is_a_site() {
    check(
        r#"<?php
function run(array $rows, string $key): mixed
{
    return [$rows['a'], $rows[$key], $rows['b']['c']];
}
"#,
        expect![[r#"
            "$rows['a']" key a
        "#]],
    );
}

#[test]
fn each_scope_answers_for_the_first_read_of_each_of_its_own_names() {
    // What a name holds is one fact, so the first read of it is where it is said. Per scope,
    // because two functions writing `$rows` are writing two variables -- and a closure is a scope,
    // so its own reads are its own.
    check(
        r#"<?php
function first(array $rows): mixed
{
    return [$rows['a'], $rows['b']];
}

function second(array $rows, array $other): callable
{
    $seen = $rows['c'];

    return function () use ($rows, $other): mixed {
        return [$rows['d'], $other['e']];
    };
}
"#,
        expect![[r#"
            "$rows['a']" key a
            "$rows['c']" key c
            "$rows['d']" key d
            "$other['e']" key e
        "#]],
    );
}

#[test]
fn a_name_the_file_builds_an_array_out_of_is_not_a_site() {
    // `$rows['a'] = 1` creates the array, which is PHP's own rule -- so nothing a resolver could
    // say about `$rows` is worth the walk it would take to say it. Every write shape counts,
    // including the append form, which is only ever a write.
    check(
        r#"<?php
function run(array $rows, string $key): mixed
{
    $rows['a'] = 1;
    $rows['b']['c'] = 2;
    $rows[] = 3;
    return [$rows['a'], $rows[$key], $rows['b']['c']];
}
"#,
        expect![""],
    );
}

#[test]
fn only_the_variable_at_the_head_of_a_chain_is_a_site() {
    // One resolution per chain rather than one per level: `$rows['b']['c']` collects the inner one,
    // whose base is the variable, and not the outer one, whose base is another subscript.
    check(
        r#"<?php
function run(array $rows): mixed
{
    return $rows['b']['c']['d'];
}
"#,
        expect![[r#"
            "$rows['b']" key b
        "#]],
    );
}

#[test]
fn a_presence_test_and_a_coalesce_are_not_sites() {
    // Both are asked because the slot may be missing, which is the opposite of a claim about it.
    check(
        r#"<?php
function run(array $rows): mixed
{
    if (isset($rows['a']) || empty($rows['b'])) {
        return 1;
    }
    return $rows['c'] ?? 2;
}
"#,
        expect![""],
    );
}

#[test]
fn a_foreach_target_is_a_write() {
    check(
        r#"<?php
function run(array $rows, array $out): array
{
    foreach ($rows as $out['seen']) {
    }
    return $out;
}
"#,
        expect![""],
    );
}

#[test]
fn a_write_inside_a_closure_is_a_write() {
    // The reads are collected scope by scope with closures among the scopes, so the writes have to
    // be too: `$rows['a'] = 1` in a closure was left out of the writes and read as an element of
    // whatever `$rows` held before it.
    check(
        r#"<?php
$build = function (?array $rows): array {
    $rows['a'] = 1;
    foreach ([1] as $rows['b']) {
    }
    return $rows;
};
$arrow = fn (?array $rows) => $rows['c'];
"#,
        expect![[r#"
            "$rows['c']" key c
        "#]],
    );
}
