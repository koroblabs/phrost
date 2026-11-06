//! A by-value call written after a by-reference one leaves the type alone.
//!
//! It used to take it away. `PathRoot::ByReference` recorded **the last call before the offset that
//! passed this name**, whether or not that call takes its argument by reference, and the reader
//! looked up exactly the one call recorded. So an ordinary call written between the out-parameter and
//! its use erased what the out-parameter left:
//!
//! ```php
//! preg_match($p, $s, $m);
//! log_it($m);        // by value
//! $m[1]              // the type is gone
//! ```
//!
//! The root's own doc-comment states the half that causes it -- "a parameter that turns out to be
//! by-value leaves the variable with no type" -- as the reason it is only reached when nothing else
//! bound the name. That reasoning is sound for *ranking* it against an assignment and does not
//! cover this: here nothing else bound the name either, and the by-value call is simply the wrong
//! one of two candidates.
//!
//! `IndexView::contract_narrowed` already does the right thing on its own route: it walks **every**
//! call that passed the value and takes the first whose parameter really is by-reference. The single
//! recorded call is the copy of that question that does not.
//!
//! The root now carries the **variable** rather than one of its calls, and the reader walks every
//! call that passed it, newest first, taking the first whose parameter really is by-reference. Two
//! details were load-bearing and neither was obvious. The path is what the root carries, because a
//! name cannot be built below the interner and a path is what `PassedTo` matches against anyway. And
//! the walk starts from the **asking position**, not from the call the root recorded: a statement the
//! offset sits inside is not one of its preceding statements, so asking just past the recorded call
//! excluded that call -- and the answer was then empty on the line straight after the
//! out-parameter, which is the line that matters most.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_call_written_between_the_out_parameter_and_its_use_leaves_the_type_alone() {
    check(
        r#"
//- /src/app.php
<?php

function fills(array &$out): void
{
    $out = [];
}

function reads(array $rows): void {}

function run(): void
{
    fills($found);
    $fo@direct@und;
    reads($found);
    $fou@after@nd;
}
"#,
        &[
            Query::hover("straight after the out-parameter", "direct"),
            Query::hover("after an ordinary call passed it on", "after"),
        ],
        expect![[r#"
            straight after the out-parameter
            - code: array $found
            - note: assigned
            after an ordinary call passed it on
            - code: array $found
            - note: assigned
        "#]],
    );
}
