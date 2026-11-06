//! `run(function () { ... // expected :) ... })->` -- a chain after a closure whose body holds a
//! comment with an unbalanced parenthesis.
//!
//! The completion scanner reads the receiver out of the characters before the cursor and counts
//! parentheses to find where the call's arguments open; the `)` in the comment was counted, the
//! arguments never balanced, and nothing was offered. Comments before the cursor are blanked first.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_parenthesis_in_a_comment_does_not_unbalance_the_call() {
    check(
        r#"
//- /src/Task.php
<?php
namespace App;

class Task
{
    public function await(): int
    {
        return 1;
    }
}

function run(\Closure $body): Task
{
    return new Task();
}

function go(): void
{
    run(function (): void {
        try {
            go();
        } catch (\Exception) {
            // nothing to do here :)
        }
        /* and (here */
    })->@here@
}
"#,
        &[Query::completion("after the closure", "here")],
        expect![[r#"
            after the closure
            - await [method]  : int  writes `();`
            - accepting the first leaves `})->await`
        "#]],
    );
}
