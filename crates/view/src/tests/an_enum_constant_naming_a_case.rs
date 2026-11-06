//! `const Default = self::Draft;` inside an enum, which is how a default case is named.
//!
//! A constant with no hint takes its type from the value beside it, and that reader is deliberately
//! narrow: it answers only for shapes the *syntax alone* settles, because a wrong type on a
//! constant that thousands of call sites pass around is worse than none. A constant naming another
//! constant is on its refusal list, on the ground that resolving a name is the def-map's job.
//!
//! One case of it needs no name resolved. Inside an enum, `self::Draft` where `Draft` is one of
//! that enum's own cases is an instance of the enum, and the cases -- gathered before the constants
//! are read, since a case may be written below a constant naming it -- settle that without leaving
//! the declaration.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_constant_that_aliases_a_case_is_the_enum() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

enum Stage: string
{
    case Draft = 'draft';
    case Live = 'live';

    const Default = self::Draft;
}

class Host
{
    public function label(): void
    {
        $stage = Stage::Default;
        $st@aliased@age;
    }
}
"#,
        &[Query::variable_class(
            "the enum the case belongs to",
            "aliased",
            "stage",
        )],
        expect![[r#"
            the enum the case belongs to
            - App\Stage
        "#]],
    );
}

/// A constant naming an ordinary constant is still refused: what it holds takes resolving a name,
/// and answering `self` there would name the enum for an `int`.
#[test]
fn a_constant_that_aliases_a_constant_is_left_alone() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

enum Stage: string
{
    case Draft = 'draft';

    const Limit = 3;
    const Cap = self::Limit;
}

class Host
{
    public function cap(): void
    {
        $cap = Stage::Cap;
        $c@plain@ap;
    }
}
"#,
        &[Query::variable_class("no class to name", "plain", "cap")],
        expect![[r#"
            no class to name
            - (type not known here)
        "#]],
    );
}

/// A list of the enum's own cases is a list of the enum -- `const ALL = [self::Draft, self::Live]`,
/// how an enum names the subset a form offers -- and one mixing a case with anything else is not.
#[test]
fn a_constant_listing_cases_is_a_list_of_the_enum() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

enum Stage: string
{
    case Draft = 'draft';
    case Live = 'live';

    const Open = [self::Draft, self::Live];
    const Mixed = [self::Draft, 'live'];
}

class Host
{
    public function label(): void
    {
        foreach (Stage::Open as $stage) {
            $st@listed@age;
        }
        foreach (Stage::Mixed as $other) {
            $ot@mixed@her;
        }
    }
}
"#,
        &[
            Query::variable_class("each listed case", "listed", "stage"),
            Query::variable_type("a list mixing a case with a string", "mixed"),
        ],
        expect![[r#"
            each listed case
            - App\Stage
            a list mixing a case with a string
            - (no type)
        "#]],
    );
}
