//! A keyed array literal written into one slot of an array, read back through both keys.
//!
//! `$rows[$id] = ['bundle' => $item->bundle(), 'count' => 1]` is how PHP builds an array of records,
//! and `$rows[$id]['bundle']` used to answer nothing. Not because either half was missing: a keyed
//! literal assigned *whole* to a variable answers for its keys, and an object written into a slot is
//! read back out of it. What failed was the two of them composed -- which is the only shape anybody
//! writes -- because a write whose target was an index could only be read as *a write into a slot of
//! the container*, and the reading that applies when the target **is** the container was never tried.
//!
//! At this layer as well as at the body's, because the body test asks `element_written` one question
//! and this asks what a reader gets: the composition runs through `slot_written`, once per step, and
//! the second step is the one that was silent.
//!
//! What is deliberately still missing is a shape that *travels*. `$x = $rows['a']; $x['bundle']` and
//! `foreach ($rows as $row) { $row['bundle'] }` both resolve the literal into a `Held` first, and
//! `Held` has no arm for a shape -- so the keys are gone by the time the next step asks. Adding one
//! is a change to the core inference vocabulary, and the two fixtures it would win do not pay for it.

use expect_test::expect;

use super::{Query, check_view};

const CLASSES: &str = r#"
//- /src/Bundle.php
<?php
namespace App;

class Bundle
{
    public function label(): string
    {
        return '';
    }
}

//- /src/Item.php
<?php
namespace App;

class Item
{
    public int $id = 0;

    public function bundle(): Bundle
    {
        return new Bundle();
    }
}
"#;

#[test]
fn a_record_written_into_a_slot_is_read_back_through_both_keys() {
    check_view(
        &[
            CLASSES,
            r#"
//- /src/Counter.php
<?php
namespace App;

class Counter
{
    public function go(Item $item): void
    {
        $rows = [];
        $rows['first'] = ['bundle' => $item->bundle(), 'count' => 1];
        $held = $rows['first']['bundle'];
        $he@after@ld;
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("the record's own key", "after"),
            Query::variable_class("and the class it names", "after", "held"),
        ],
        expect![[r#"
            the record's own key
            - Bundle
            and the class it names
            - App\Bundle
        "#]],
    );
}

#[test]
fn a_variable_key_is_still_the_slot_the_read_asks_about() {
    // The commonest spelling: the key is computed, so neither the write nor the read can name it,
    // and both mean "whichever slot that is". The record's own keys are named, which is the half
    // that answers.
    check_view(
        &[
            CLASSES,
            r#"
//- /src/Counter.php
<?php
namespace App;

class Counter
{
    public function go(Item $item): void
    {
        $rows = [];
        $rows[$item->id] = ['bundle' => $item->bundle(), 'count' => 1];
        $held = $rows[$item->id]['bundle'];
        $he@after@ld;
    }
}
"#,
        ]
        .concat(),
        &[Query::variable_type("through a computed key", "after")],
        expect![[r#"
            through a computed key
            - Bundle
        "#]],
    );
}

#[test]
fn a_slot_the_write_did_not_fill_answers_nothing() {
    // The exactness that makes the reading safe: `$rows['first']` says nothing about
    // `$rows['second']`, and a literal key on both sides is the one case where that can be checked.
    check_view(
        &[
            CLASSES,
            r#"
//- /src/Counter.php
<?php
namespace App;

class Counter
{
    public function go(Item $item): void
    {
        $rows = [];
        $rows['first'] = ['bundle' => $item->bundle()];
        $held = $rows['second']['bundle'];
        $he@after@ld;
    }
}
"#,
        ]
        .concat(),
        &[Query::variable_type("another slot", "after")],
        expect![[r#"
            another slot
            - (no type)
        "#]],
    );
}
