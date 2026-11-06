//! Writes that bind a local without being a write to the local.
//!
//! PHP creates a variable by being written *through*, not only by being written: `$rows['a'] = 1`
//! is where `$rows` comes from, and it is how most arrays in PHP come into being. Both of these
//! bound nothing, because both places that look asked whether the target *was* a variable.

use expect_test::expect;

use crate::tests::check;

#[test]
fn writing_an_element_creates_the_array_it_is_written_through() {
    check(
        r#"
<?php
function properties(): array {
    $rows['type'] = 'string';
    $rows['label'] = 'Label';
    return $ro$0ws;
}
"#,
        "rows",
        // `array`, not the type of what was written: the value went into a slot, and saying more
        // would be a guess about a key nobody read back.
        expect!["(array)"],
    );
}

#[test]
fn appending_creates_it_too() {
    check(
        r#"
<?php
function collect(): array {
    $found[] = 1;
    return $fou$0nd;
}
"#,
        "found",
        expect!["(array)"],
    );
}

#[test]
fn a_nested_element_write_still_creates_the_outermost_variable() {
    check(
        r#"
<?php
function grid(): array {
    $cells['a']['b'] = 1;
    return $cel$0ls;
}
"#,
        "cells",
        expect!["(array)"],
    );
}

#[test]
fn a_property_written_through_an_element_binds_no_local() {
    check(
        r#"
<?php
class Store
{
    private array $rows = [];

    public function add(): void
    {
        $this->rows['a'] = 1;
        $unrelated = $ro$0ws;
    }
}
"#,
        "rows",
        expect!["<not declared here>"],
    );
}

#[test]
fn a_foreach_by_reference_binds_its_value() {
    check(
        r#"
<?php
class Item { public function touch(): void {} }

/** @param array<Item> $items */
function touch_all(array $items): void {
    foreach ($items as &$item) {
        $it$0em->touch();
    }
}
"#,
        "item",
        expect!["$items[element]"],
    );
}

#[test]
fn an_assignment_by_reference_still_names_its_target() {
    check(
        r#"
<?php
function alias(array $rows): void {
    $first = &$rows;
    $co$0py = $first;
}
"#,
        "first",
        expect!["$rows"],
    );
}

#[test]
fn reading_an_element_is_one_step_from_what_holds_it() {
    // The same step a `foreach` takes, which is why it is worth taking: wherever the base has an
    // element type to give -- a `@param array<Item>`, a method returning `list<Item>` -- reading
    // one element resolves for free.
    check(
        r#"
<?php
class Item {}

/** @param array<Item> $items */
function first(array $items): void {
    $one = $items[0];
    $o$0ne;
}
"#,
        "one",
        expect!["$items['0']"],
    );
}

#[test]
fn an_element_write_does_not_outrank_the_assignment_that_declared_it() {
    // `$rows[] = x` after `$rows = [...]` is a mutation, not a declaration. Taking the latest write
    // of any kind sent goto to the mutation and left the real declaration unreachable -- PHP's own
    // rule is that the element write creates the variable only when nothing else did.
    // The two answers have to differ for this to test anything: the declaration says `$seed`, and
    // taking the element write instead would read its value and say `string`.
    check(
        r#"
<?php
function build(array $seed): void {
    $rows = $seed;
    $rows[] = 'later';
    $ro$0ws;
}
"#,
        "rows",
        expect!["$seed"],
    );
}

#[test]
fn a_static_declaration_binds_before_anything_is_written_to_it() {
    // The whole idiom: the guard reads the variable above the first write, so a search that only
    // knows about writes has nothing to say exactly where a reader asks.
    check(
        r#"
<?php
function options(): array {
    static $options;
    if (empty($options)) {
        $options['a'] = 1;
    }
    return $opt$0ions;
}
"#,
        "options",
        expect!["(array)"],
    );
}

#[test]
fn an_initialised_static_holds_what_it_was_initialised_with() {
    check(
        r#"
<?php
class Item {}

function one(): Item {
    static $one = new Item();
    return $o$0ne;
}
"#,
        "one",
        expect!["new Item"],
    );
}

#[test]
fn a_global_binds_the_name_it_imports() {
    check(
        r#"
<?php
function url(): string {
    global $base_url;
    return (string) $base$0_url;
}
"#,
        "base_url",
        // Bound, and with no type: nothing in this file says what the global holds, and inventing
        // one would be the guess.
        expect!["<not declared here>"],
    );
}

#[test]
fn a_variadic_parameter_holds_an_array_of_its_type() {
    // The hint describes one argument and the variable holds all of them, so reading the hint as the
    // variable's type made `foreach ($definitions as $one)` yield nothing at all.
    check(
        r#"
<?php
class Set
{
    public function __construct(Definition ...$definitions)
    {
        $defin$0itions;
    }
}
"#,
        "definitions",
        expect!["variadic Definition"],
    );
}
