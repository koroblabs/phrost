//! `$row = $this->find(); if ($c) { $row = new Row(); }` holds a `?Row`, and it held a `Row`.
//!
//! A join of operands that all name one class is reduced to one of them, because `Row|Row` is a
//! union nobody wrote and every reader that takes a union apart pays for an operand that adds
//! nothing. The operand kept was the first, which is a write -- and `?Row` beside `Row` names one
//! class, so the reduction was throwing the null away on the path the branch did not take. Nothing
//! failed: the answer looked *more* specific, which is why it survived.
//!
//! Keeping the nullable operand instead turns the reduction back into a claim about the class, and
//! that is all it was ever meant to be. `??` and `?:` are unaffected -- their left operand has its
//! null taken off by the operator before the reduction runs.
//!
//! What the fix uncovered is that the reduction had been hiding four other rules, each of which
//! answered `Unproved` where it had something to say. They are the rest of this file, and each one
//! is about the same thing: what the path that *skipped* the write is known to satisfy.

use expect_test::expect;

use super::diagnostics::check;

/// A nullable source, a class to write, and something that refuses a null.
const ROWS: &str = r#"
//- /src/Rows.php
<?php

class Row
{
    public function id(): int
    {
        return 0;
    }
}

class Rows
{
    public function find(): ?Row
    {
        return NULL;
    }

    public function takes(Row $row): int
    {
        return $row->id();
    }
}
"#;

#[test]
fn a_write_in_a_branch_joins_the_nullable_it_did_not_replace() {
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

function run(Rows $rows, bool $flag): int
{{
    $row = $rows->find();

    if ($flag) {{
        $row = new Row();
    }}

    return $rows->takes($row);
}}
"#
        ),
        expect![[r#"
            11:25-11:29 warning [possibly-null-argument] $row takes \Row, and this may be null
        "#]],
    );
}

#[test]
fn a_coalesce_still_reduces_to_the_one_class_it_names() {
    // The control, and the reason the reduction exists: `??` consumed the left operand's null, so
    // both sides are a `Row` and the answer is a `Row` rather than a `Row|Row`.
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

function run(Rows $rows): int
{{
    $row = $rows->find() ?? new Row();

    return $rows->takes($row);
}}
"#
        ),
        expect![""],
    );
}

#[test]
fn the_path_that_skipped_the_write_is_read_through_an_or() {
    // Entering `if ($best === NULL || $row->id() < $best->id())` proves nothing, because either
    // disjunct may be the one that held -- while *skipping* it proves `$best !== NULL`, because
    // both halves failed. Complementing what entering proved therefore answered `Unproved` for
    // every find-the-best loop in PHP; reading the condition at the skipped polarity answers it.
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

function run(Rows $rows, Row $candidate): int
{{
    $best = $rows->find();

    if ($best === NULL || $candidate->id() < $best->id()) {{
        $best = $candidate;
    }}

    return $rows->takes($best);
}}
"#
        ),
        expect![""],
    );
}

#[test]
fn a_guard_between_the_write_and_the_end_of_its_branch_is_what_the_branch_contributes() {
    // What a branch hands the join is the value where control *leaves* it, not what the assignment
    // put there. Symfony's `UniqueEntityValidator` writes it: one arm takes a manager that cannot
    // be null, the other takes one that can and throws if it is.
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

function run(Rows $rows, bool $flag): int
{{
    if ($flag) {{
        $row = $rows->find();

        if (!$row) {{
            return 0;
        }}
    }}
    else {{
        $row = new Row();
    }}

    return $rows->takes($row);
}}
"#
        ),
        expect![""],
    );
}

/// The same fixture without the guard, which is what proves the silence above is the rule and not
/// the fixture: both arms of the `if`/`else` write, so nothing before the construct reaches the
/// read and the only operand that can carry a null is the one the guard was cleaning up.
#[test]
fn and_the_same_branch_without_the_guard_hands_the_join_its_null() {
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

function run(Rows $rows, bool $flag): int
{{
    if ($flag) {{
        $row = $rows->find();
    }}
    else {{
        $row = new Row();
    }}

    return $rows->takes($row);
}}
"#
        ),
        expect![[r#"
            12:25-12:29 warning [possibly-null-argument] $row takes \Row, and this may be null
        "#]],
    );
}

/// A guard whose claim is written on the callee, which the skipped path could not state.
///
/// WordPress writes `if (!is_wp_error($thing)) { $thing = new WP_Error(); }` and tags `is_wp_error`
/// with `@phpstan-assert-if-true`. The vocabulary a skipped path is stated in has a null, a class
/// and a kind in it and nothing for a tag, so this answered "nothing proved" and the join put back
/// a null the guard had ruled out. The claim travels as a path instead, and the reader resolves it.
///
/// One claim only, which is the honest limit: `if (!$rows || is_wrong($rows))` proves two things
/// about the path around it and only one of them fits.
#[test]
fn a_guard_that_names_what_it_proves_on_the_callee_is_carried_as_a_path() {
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

/**
 * @phpstan-assert-if-true Row $thing
 */
function is_a_row(mixed $thing): bool
{{
    return $thing instanceof Row;
}}

function run(Rows $rows): int
{{
    $row = $rows->find();

    if (!is_a_row($row)) {{
        $row = new Row();
    }}

    return $rows->takes($row);
}}
"#
        ),
        expect![""],
    );
}

/// And the same callee with its tag taken off, which is what proves the silence above is the tag
/// being read rather than the guard being ignored.
#[test]
fn and_the_same_guard_with_no_tag_on_the_callee_proves_nothing() {
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

function is_a_row(mixed $thing): bool
{{
    return $thing instanceof Row;
}}

function run(Rows $rows): int
{{
    $row = $rows->find();

    if (!is_a_row($row)) {{
        $row = new Row();
    }}

    return $rows->takes($row);
}}
"#
        ),
        expect![[r#"
            16:25-16:29 warning [possibly-null-argument] $row takes \Row, and this may be null
        "#]],
    );
}

#[test]
fn a_write_inside_a_loop_says_nothing_about_the_path_that_skipped_it() {
    // The refusal the three rules above need. A guard inside a loop describes the pass it ran on,
    // and the path that reaches the read without the write is the one where the body never ran --
    // so the value from before the loop gets through with nothing proved about it.
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

/** @param list<Row> $candidates */
function run(Rows $rows, array $candidates): int
{{
    $best = $rows->find();

    foreach ($candidates as $candidate) {{
        if ($best === NULL) {{
            $best = $candidate;
        }}
    }}

    return $rows->takes($best);
}}
"#
        ),
        expect![[r#"
            14:25-14:30 warning [possibly-null-argument] $row takes \Row, and this may be null
        "#]],
    );
}

#[test]
fn a_condition_that_is_an_assignment_still_states_what_it_tested() {
    // `if ($isNew = null === $handler) { $handler = new static(); }` is PHP's way of asking a
    // question and remembering the answer, and Symfony's `ErrorHandler::register` is written that
    // way. The guard machinery matched the assignment only when the *assigned* name was the
    // subject, so a test about another value read as no test at all.
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

function run(Rows $rows): int
{{
    $row = $rows->find();

    if ($isNew = NULL === $row) {{
        $row = new Row();
    }}

    return $rows->takes($row) + (int) $isNew;
}}
"#
        ),
        expect![""],
    );
}
