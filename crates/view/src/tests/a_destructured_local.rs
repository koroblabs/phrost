//! What `[$a, $b] = $row` gives each name.
//!
//! A destructuring target writes through no variable of its own, so the write was recorded for
//! nobody and every name it binds had no type at all -- `[$startTime, $endTime] = [Carbon::parse(..),
//! Carbon::parse(..)]` and the 922 other destructurings in one framework's `src/` among them. What
//! settles each name is the slot it took, which lowering writes on the target's elements (holes
//! counted) so that a reader reads it rather than counting.
//!
//! A name takes a slot only as a *whole*. `[$parts[0], $parts[1]] = $pair` writes two slots of
//! `$parts` and says nothing about `$parts` itself, and reading it as that name's value retyped the
//! array as one of its own elements -- `implode(' ', $parts)` reported on a line that works.
//!
//! # A `foreach` target reads the slot, unless the subject is a literal of rows
//!
//! `foreach ($rows as [$row, $label])` reads the slot of the subject's *element*, which is right
//! exactly when the element states its slots: `@return array<int, array{0: Duo, 1: string}>` says
//! what each pass holds, and the shape travels.
//!
//! The refusal this used to make wholesale was right for one shape and is kept for it. A table of
//! test cases written as a literal -- `[[$handler, Stage::A, [Stage::B]], ...]` -- has one element
//! type for all its rows, so a slot of it is the union of everything in every row: each name gets
//! all three, and a union carrying an alternative the parameter refuses reports the call that
//! passes the one that fits. Two findings on one real project's tests came from exactly that.
//!
//! Where the right-hand side is a literal the slot is read from *it*, exactly:
//! `[$name, $count] = [$row->name(), 0]` gives `$count` the `0`. Reading it as `<literal>[1]` would
//! answer the literal's element type, which is the union of everything in it -- wider than the
//! truth, and a union carrying an alternative the parameter refuses reports the code that passes the
//! one that fits.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /src/Rows.php
<?php
namespace App;

class Duo
{
    public function id(): int {}
}

class Rows
{
    /** @return array{0: Duo, 1: string} */
    private function pair(): array {}

    /** @return Duo[] */
    private function many(): array {}

    public function fromLiteral(): void
    {
        [$first, $second] = [new Duo(), 'plain'];
        $fi@literal@rst;
    }

    public function fromShape(): void
    {
        [$left, $right] = $this->pair();
        $le@shape@ft;
    }

    public function pastAHole(): void
    {
        list($a, , $third) = ['one', 2, new Duo()];
        $th@hole@ird;
    }

    public function nested(): void
    {
        [[$inner], $outer] = [[new Duo()], 'plain'];
        $in@nested@ner;
    }

    /** @param string[] $parts */
    public function intoSlots(array $parts): void
    {
        [$parts[0], $parts[1]] = $this->pair();
        $pa@slots@rts;
    }

    public function overALoop(): void
    {
        foreach ($this->rows() as [$row, $label]) {
            $r@loop@ow;
        }
    }

    /** @return array<int, array{0: Duo, 1: string}> */
    private function rows(): array {}
}
"#;

/// Every shape at once, because the bug was that none of them answered: a destructuring is one
/// construct and the slot is the only thing that differs between these.
#[test]
fn each_name_a_destructuring_binds_takes_its_own_slot() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("from a literal", "literal"),
            Query::variable_type("from a shape the callee declares", "shape"),
            Query::variable_type("past a hole, which takes its place", "hole"),
            Query::variable_type("from a nested target", "nested"),
            Query::variable_type("a target that writes slots of another array", "slots"),
            Query::variable_type(
                "from the rows of a loop, whose element states its slots",
                "loop",
            ),
        ],
        expect![[r#"
            from a literal
            - Duo
            from a shape the callee declares
            - Duo
            past a hole, which takes its place
            - Duo
            from a nested target
            - Duo
            a target that writes slots of another array
            - string[]
            from the rows of a loop, whose element states its slots
            - Duo
        "#]],
    );
}

/// And a literal of rows is still refused, for the reason the module note gives.
#[test]
fn a_loop_over_a_literal_of_rows_reads_no_slot() {
    check_view(
        r#"
//- /src/app.php
<?php

class Duo
{
    public function label(): string {}
}

class Repo
{
    public function overALiteral(): void
    {
        foreach ([[new Duo(), 'plain'], ['other', 'plain']] as [$row, $label]) {
            $r@loop@ow;
        }
    }
}
"#,
        &[Query::variable_type(
            "one element type for every row",
            "loop",
        )],
        expect![[r#"
            one element type for every row
            - (no type)
        "#]],
    );
}
