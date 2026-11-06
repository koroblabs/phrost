//! `match (true)` read as the `if`/`elseif` chain it is.
//!
//! An arm runs when its own condition matched the subject and every earlier one did not, so both
//! halves narrow: the arm's own condition as written, and each earlier arm's negated. Only where
//! the subject is the literal -- any other subject compares values, and `match ($kind) { 'a' => }`
//! proves a scalar this layer has no vocabulary for.

use expect_test::expect;

use super::check;

#[test]
fn an_arm_narrows_by_its_own_condition() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $text): string
    {
        return match (true) {
            $text !== null => $te$0xt,
            default => 'none',
        };
    }
}
"#,
        "text",
        expect!["not null $text"],
    );
}

#[test]
fn a_later_arm_narrows_by_the_earlier_ones_failing() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $text): string
    {
        return match (true) {
            $text === null => 'none',
            default => $te$0xt,
        };
    }
}
"#,
        "text",
        expect!["not null $text"],
    );
}

/// The innermost claim wins, which here means the arm's own condition over the earlier negations.
#[test]
fn an_arm_past_two_others_reads_its_own_condition() {
    check(
        r#"<?php
class Base {}
class Special extends Base {}
class Repo
{
    public function run(?Base $value): string
    {
        return match (true) {
            $value === null => 'none',
            $value instanceof Special => $val$0ue,
            default => 'other',
        };
    }
}
"#,
        "value",
        expect!["instanceof Special $value"],
    );
}

/// An arm listing two conditions proves nothing of its own -- either may be the one that matched.
#[test]
fn an_arm_with_two_conditions_proves_neither() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $text, bool $flag): string
    {
        return match (true) {
            $text !== null, $flag => $te$0xt,
            default => 'none',
        };
    }
}
"#,
        "text",
        expect!["?string"],
    );
}

/// A subject that is not the literal compares values, and no arm is a condition about the subject.
#[test]
fn a_match_over_a_value_is_not_a_chain_of_guards() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $text, string $kind): string
    {
        return match ($kind) {
            'a' => $te$0xt ?? '',
            default => 'none',
        };
    }
}
"#,
        "text",
        expect!["?string"],
    );
}
