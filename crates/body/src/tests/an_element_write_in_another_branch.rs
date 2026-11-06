//! A write into a slot counts only where control could have gone through it.
//!
//! Every other reader of a write asks `facts.reach_of` first -- a write in a branch the position is
//! not in did not run, which is the whole of what `writes_in_a_branch` pins for a local. This walk
//! did not ask at all: it took every assignment written before the offset, so an `if`/`elseif` chain
//! that rewrites `$callback` in one arm answered with that arm's value in the arm beside it.
//!
//! Nine findings on two corpora, and one arrival: a container written in several branches has no one
//! answer, and the last *reaching* write is a different guess from the last written one rather than a
//! better-founded one.

use expect_test::expect;

use super::array_elements::check_element;

#[test]
fn the_branch_the_position_is_in_is_the_one_that_answers() {
    check_element(
        r#"<?php
class Repo
{
    public function run(bool $c): void
    {
        $rows = [];
        if ($c) {
            $rows['a'] = new Pen();
            $ro$0ws;
        } else {
            $rows['a'] = new Pencil();
        }
    }
}
"#,
        "rows",
        Some("a"),
        expect!["new Pen"],
    );
}

#[test]
fn and_the_other_branch_answers_with_its_own() {
    check_element(
        r#"<?php
class Repo
{
    public function run(bool $c): void
    {
        $rows = [];
        if ($c) {
            $rows['a'] = new Pen();
        } else {
            $rows['a'] = new Pencil();
            $ro$0ws;
        }
    }
}
"#,
        "rows",
        Some("a"),
        expect!["new Pencil"],
    );
}

/// Past the whole construct both writes reach, and the later one is the answer -- which is what the
/// walk already did and what a reader of one slot has to live with.
#[test]
fn past_the_construct_the_later_write_still_wins() {
    check_element(
        r#"<?php
class Repo
{
    public function run(bool $c): void
    {
        $rows = [];
        if ($c) {
            $rows['a'] = new Pen();
        } else {
            $rows['a'] = new Pencil();
        }
        $ro$0ws;
    }
}
"#,
        "rows",
        Some("a"),
        expect!["new Pencil"],
    );
}
