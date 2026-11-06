//! A positional literal keeps its slots, so destructuring one reads the slot rather than the union.
//!
//! `foreach ($cases as [$handler, $stage, $rest])` over an array of three-element literals gave
//! every name the union of all three -- `Handler|Stage|Stage[]` -- so the call that took the stage
//! was reported against a value it could never hold. A test's data provider is written this way and
//! so is half of PHP's own idiom for a pair.
//!
//! The rule is PHP's own keying and not a second one: a bare entry takes the next index and a
//! literal integer key moves the counter past it, which is the same statement `['a' => 1]` already
//! made about a named slot. See `phrost_view::tests::list_literal`.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn each_name_a_destructure_binds_reads_its_own_slot() {
    check(
        r#"
//- /src/app.php
<?php

enum Stage: string
{
    case Shipped = 'shipped';
}

class Handler {}

class Cases
{
    public function takesStage(Stage $stage): void {}

    public function run(): void
    {
        $cases = [
            [new Handler(), Stage::Shipped, [Stage::Shipped]],
            [new Handler(), Stage::Shipped, [Stage::Shipped]],
        ];

        foreach ($cases as [$handler, $stage, $rest]) {
            $this->takesStage($stage);
            $ha@handler@ndler;
            $st@stage@age;
            $re@rest@st;
        }
    }
}
"#,
        &[
            Query::hover("the first slot", "handler"),
            Query::hover("the second slot", "stage"),
            Query::hover("the third slot", "rest"),
        ],
        expect![[r#"
            the first slot
            - code: Handler $handler
            - note: loop variable
            the second slot
            - code: Stage $stage
            - note: loop variable
            the third slot
            - code: array{0: Stage} $rest
            - note: loop variable
        "#]],
    );
}
