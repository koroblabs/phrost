//! A write into a slot is the element type only where the container started here.
//!
//! `$rows = []; $rows[] = $user;` is the shape `elements.rs` was written for, and there the writes
//! really are every word about what the array holds. `$calls = $value->getMethodCalls(); $calls[] =
//! [$constructor, $args];` is the same syntax and the opposite fact: the push says what was appended
//! and nothing about what the call had already put there, so reading it as *the* element type states
//! a type narrower than the truth -- and a type narrower than the truth reports correct code.
//!
//! Two sources say the container was filled elsewhere: an assignment of anything but a literal, and
//! the parameter list, which is the one source no statement in the scope shows. Both cost the whole
//! reading rather than joining with it, because what else is in there is not a type this can name.
//!
//! Measured: **83 findings gone across three corpora and none added**. WordPress's exporter is the
//! clearest of them -- `$authors[] = get_userdata(..)` pushes a `WP_User|false`, `$authors =
//! array_filter($authors)` takes the falses out, and six lines were reported for a `false` that
//! cannot reach them.

use expect_test::expect;

use super::array_elements::check_element;

#[test]
fn a_container_that_started_as_a_literal_is_what_its_writes_say() {
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $rows = [];
        $rows[] = new Row();
        $ro$0ws;
    }
}
"#,
        "rows",
        None,
        expect!["new Row"],
    );
}

#[test]
fn a_container_assigned_from_a_call_says_nothing_about_what_is_in_it() {
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $rows = $this->existing();
        $rows[] = new Row();
        $ro$0ws;
    }
}
"#,
        "rows",
        None,
        expect!["<nothing written>"],
    );
}

#[test]
fn a_parameter_arrives_full_and_a_write_into_it_describes_one_slot() {
    check_element(
        r#"<?php
class Repo
{
    public function run(array $rows): void
    {
        $rows[] = new Row();
        $ro$0ws;
    }
}
"#,
        "rows",
        None,
        expect!["<nothing written>"],
    );
}

/// The literal reading is untouched: it says what the *whole* array is, so where it came from is not
/// a question about it.
#[test]
fn a_literal_written_over_a_foreign_array_still_answers() {
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $rows = $this->existing();
        $rows = [new Row()];
        $ro$0ws;
    }
}
"#,
        "rows",
        None,
        expect!["new Row"],
    );
}
