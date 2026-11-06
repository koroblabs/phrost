//! What a piped value holds is what the last stage returns.
//!
//! The corpus shape: `$title |> trim(...) |> createDate(...)` and then `$result->`. Lowered as the
//! call each stage makes, so hover on the result is the last callee's return type and completion
//! after it offers that class's members, with nothing taught about pipes above lowering.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn hover_and_completion_read_the_last_stages_return() {
    check(
        r#"
//- /src/app.php
<?php

class Stamp
{
    public function getTimestamp(): int { return 0; }
}

function createDate(string $input): Stamp
{
    return new Stamp();
}

function run(): void
{
    $title = 'hello';
    $result = $title
        |> trim(...)
        |> createDate(...);
    $res@hover@ult->@complete@;
}
"#,
        &[
            Query::hover("the piped result", "hover"),
            Query::completion("after the arrow", "complete"),
        ],
        expect![[r#"
            the piped result
            - code: Stamp $result
            - note: assigned
            after the arrow
            - getTimestamp [method]  : int  writes `()`
            - accepting the first leaves `$result->getTimestamp;`
        "#]],
    );
}
