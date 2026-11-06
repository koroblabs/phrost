//! `global $x;` inside a function, and what the name it imports holds.
//!
//! The statement introduces a name without writing to it, which is why both searches have to know
//! about it: one says the name exists, the other says what it holds, and until now only the first
//! did. `$config->` inside the function offered nothing at all.

use expect_test::expect;

use super::{check, check_names};

#[test]
fn a_global_holds_what_the_file_scope_assigned_it() {
    check(
        r#"
<?php
class Config {}

$config = new Config();

function test(): void {
    global $config;
    $conf$0ig;
}
"#,
        "config",
        expect!["new Config"],
    );
}

#[test]
fn the_name_exists_as_well_as_holding_something() {
    // The other search. `chain_root` goes through this one before it can use the binding, so a
    // construct known only to the binding search answers nowhere a completion is asked for.
    check_names(
        r#"
<?php
class Config {}

$config = new Config();

function test(): void {
    global $config;
    $0
}
"#,
        expect!["$config"],
    );
}

#[test]
fn a_top_level_assignment_written_below_the_function_still_reaches_it() {
    // The function runs when it is called, which is after the whole script has been read -- so the
    // usual "written before the cursor" rule is the wrong one for a global.
    check(
        r#"
<?php
class Config {}

function test(): void {
    global $config;
    $conf$0ig;
}

$config = new Config();
"#,
        "config",
        expect!["new Config"],
    );
}

#[test]
fn a_write_inside_the_function_wins_over_the_file_scope() {
    // It is the same variable, and the write is the later claim about it.
    check(
        r#"
<?php
class Config {}
class Other {}

$config = new Config();

function test(): void {
    global $config;
    $config = new Other();
    $conf$0ig;
}
"#,
        "config",
        expect!["new Other"],
    );
}

#[test]
fn an_annotation_at_file_scope_says_what_the_global_holds() {
    // The idiom a bootstrap file is written in: the assignment produces something too general, and
    // the comment above it is the only place the real type is written down.
    check(
        r#"
<?php
class Config {}

/** @var Config $config */
$config = make();

function test(): void {
    global $config;
    $conf$0ig;
}
"#,
        "config",
        expect!["Config"],
    );
}

#[test]
fn a_global_the_file_scope_never_assigns_holds_nothing() {
    // Unchanged, and deliberately so: the variable comes from another file, and inventing a type
    // for it would be the guess. The name is still in scope -- see the second search above.
    check(
        r#"
<?php
function url(): string {
    global $base_url;
    return (string) $base$0_url;
}
"#,
        "base_url",
        expect!["<not declared here>"],
    );
}

#[test]
fn a_global_at_file_scope_is_not_a_search_of_its_own() {
    // `global` at the top level names the variable that is already there, so resolving it against
    // the file scope would be the same search going round again.
    check(
        r#"
<?php
class Config {}

global $config;
$config = new Config();
$conf$0ig;
"#,
        "config",
        expect!["new Config"],
    );
}
