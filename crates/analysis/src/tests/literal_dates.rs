//! `strtotime()` with the date written at the call.
//!
//! The end of the recogniser in `phrost_view::literal_returns`, asserted where it matters: whether
//! a checker reports the failure branch. A literal it reads cannot fail; a value out of an array
//! can, and still does.
//!
//! The count may be written as a hole, which is how an application spells the same thing --
//! `strtotime("-$i months")`, fifteen times on one real checkout and never once in WordPress,
//! Laravel, Symfony or Slim. What decides is the hole's type and where it sits: PHP reads
//! `-3 months`, `--3 months` and `-0 months` alike, so any integer in a count slot leaves a date,
//! and nothing about a type promises a value spells `months` or four digits of a year.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_date_written_at_the_call_cannot_fail_and_one_read_from_data_can() {
    check(
        r#"
//- /src/stub.php
<?php

function strtotime(string $datetime, ?int $baseTimestamp = null): int|false {}

//- /src/app.php
<?php

class Report
{
    public function run(array $row): void
    {
        $written = strtotime('-25 days');
        $fromData = strtotime($row['date']);
        $typo = strtotime('yesterdya');
        $wri@written@tten;
        $fro@data@mData;
        $ty@typo@po;
    }
}
"#,
        &[
            Query::hover("written at the call", "written"),
            Query::hover("read out of an array", "data"),
            Query::hover("a format it will not vouch for", "typo"),
        ],
        expect![[r#"
            written at the call
            - code: int $written
            - note: assigned
            read out of an array
            - code: int|false $fromData
            - note: assigned
            a format it will not vouch for
            - code: int|false $typo
            - note: assigned
        "#]],
    );
}

#[test]
fn a_count_written_as_a_hole_cannot_fail_when_the_hole_holds_an_integer() {
    check(
        r#"
//- /src/stub.php
<?php

function strtotime(string $datetime, ?int $baseTimestamp = null): int|false {}

//- /src/app.php
<?php

class Schedule
{
    public function run(int $months, ?int $maybe, string $unit, int $y, int $m, int $d): void
    {
        $counted = strtotime("-$months months");
        $nullable = strtotime("-$maybe months");
        $spelled = strtotime("$y-$m-$d");
        $unitFromAValue = strtotime("$months $unit");
        $cou@counted@nted;
        $nul@nullable@lable;
        $spe@spelled@lled;
        $uni@unit@tFromAValue;
    }
}
"#,
        &[
            Query::hover("an integer in the count slot", "counted"),
            Query::hover("a nullable there, which renders nothing", "nullable"),
            Query::hover("holes spelling a date instead of counting", "spelled"),
            Query::hover("a unit read out of a value", "unit"),
        ],
        expect![[r#"
            an integer in the count slot
            - code: int $counted
            - note: assigned
            a nullable there, which renders nothing
            - code: int|false $nullable
            - note: assigned
            holes spelling a date instead of counting
            - code: int|false $spelled
            - note: assigned
            a unit read out of a value
            - code: int|false $unitFromAValue
            - note: assigned
        "#]],
    );
}
