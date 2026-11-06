//! A `match` over an enum with no arm for one of its cases.
//!
//! `UnhandledMatchError` is the one place PHP turns a missing case into a crash rather than a
//! default, and it is the failure a *new* case creates: adding one breaks every `match` over that
//! enum written without a `default`.
//!
//! The case a `match` leaves out is often handled before it -- an early return, or a guard that
//! rules it out -- and reading either would need narrowing over enum *cases*, which is a value
//! rather than a class or a null and has no vocabulary here. What the body can say is weaker and
//! enough: a case it mentions anywhere has been thought about, and what this check is for is the
//! case nobody thought about.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_case_no_arm_names_is_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

enum Stage: string
{
    case Draft = 'draft';
    case Live = 'live';
    case Done = 'done';
}

class Host
{
    public function label(Stage $stage): string
    {
        return match ($stage) {
            Stage::Draft => 'd',
            Stage::Live => 'l',
        };
    }
}
"#,
        expect![[r#"
            15:23-15:29 error [unhandled-case] App\Stage has no arm for Done, and throws UnhandledMatchError for it
        "#]],
    );
}

/// A `default` handles everything left, which makes the question moot.
#[test]
fn a_default_arm_answers_for_the_rest() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

enum Stage: string
{
    case Draft = 'draft';
    case Live = 'live';
}

class Host
{
    public function label(Stage $stage): string
    {
        return match ($stage) {
            Stage::Draft => 'd',
            default => 'x',
        };
    }
}
"#,
        expect![""],
    );
}

/// The case the body rules out before the `match` -- here by an early return -- is a case the
/// author wrote down, and the value cannot reach the arms as that one.
#[test]
fn a_case_the_body_names_elsewhere_is_taken_as_handled() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

enum Stage: string
{
    case Draft = 'draft';
    case Live = 'live';
    case Done = 'done';
}

class Host
{
    public function label(Stage $stage): string
    {
        if ($stage === Stage::Done) {
            return 'o';
        }

        return match ($stage) {
            Stage::Draft => 'd',
            Stage::Live => 'l',
        };
    }
}
"#,
        expect![""],
    );
}

/// The same mention spelled by what the case is backed by: `->value === 'done'` rules out the case
/// whose value is `'done'`, read off the enum's own declaration. A literal no case is backed by
/// rules out nothing, and the case it was meant for is still reported.
#[test]
fn a_case_ruled_out_by_its_backing_value_is_taken_as_handled() {
    check(
        r#"
//- /src/Stage.php
<?php
namespace App;

enum Stage: string
{
    case Draft = 'draft';
    case Live = 'live';
    case Done = "done";
}

//- /src/app.php
<?php
namespace App;

class Host
{
    public function label(Stage $stage): string
    {
        if ($stage->value === 'done') {
            return 'o';
        }

        return match ($stage) {
            Stage::Draft => 'd',
        };
    }
}
"#,
        expect![[r#"
            12:23-12:29 error [unhandled-case] App\Stage has no arm for Live, and throws UnhandledMatchError for it
        "#]],
    );
}
