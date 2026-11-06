//! `if ($x instanceof Month) { $x = $x->value; }` -- the complement the class direction was missing.
//!
//! The three guard directions each have two sides, and only two of them had both. Null has
//! `NotNull` and `Null`; a kind has `OfKind` and `NotOfKind`; a class had only `OfClass`, read for
//! `if (!$x instanceof T) { $x = ...; }`, and nothing for the same guard written the other way
//! round. So the commonest normalising shape in a typed library -- `int|Month` down to an `int` --
//! put the class back beside the value the branch had just replaced it with, and the call below it
//! reported `takes int, and this is int|Month`.
//!
//! What is still one claim and not a conjunction: a write nested inside *several* guards has one
//! skipped path and it satisfies the complement of every one of them. `Otherwise` carries one, and
//! the innermost is what it gets -- so `if (!$file instanceof File) { if ($file instanceof
//! SplFileInfo) { ... } else { ... } }` reads the inner complement and loses the outer one. Four
//! reports on one framework, against fifteen the complement removes.

use expect_test::expect;

use super::check;

#[test]
fn a_branch_entered_because_the_value_was_of_a_class_is_skipped_where_it_was_not() {
    check(
        r#"<?php
class Month {}

class Repo
{
    public function run(int|Month $month): void
    {
        if ($month instanceof Month) {
            $month = $month->value;
        }
        $mon$0th;
    }
}
"#,
        "month",
        expect!["$month->$value or $month before minus Month"],
    );
}

/// The other side, which already worked, so the two are pinned against each other.
#[test]
fn and_the_guard_written_the_other_way_round_still_says_its_own_thing() {
    check(
        r#"<?php
class Month {}

class Repo
{
    public function run(int|Month $month): void
    {
        if (!$month instanceof Month) {
            $month = new Month();
        }
        $mon$0th;
    }
}
"#,
        "month",
        expect!["new Month or $month before as Month"],
    );
}
