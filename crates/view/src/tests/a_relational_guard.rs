//! `if ($count > 0)` rules null out, and `if ($count < 5)` does not.
//!
//! A relational comparison lowered to a bare bool-producing node could not say which comparison it
//! was, so none of them narrowed anything -- and `> 0` is how a nullable count is guarded. The rule
//! is only sound in two of the four directions, and PHP is the reason: `null` compares as `false`
//! against a scalar and as `""` against a string, so `null > x` is false whatever `x` is and
//! `null <= x` is true whatever `x` is, while `<` and `>=` turn on whether `x` is falsy -- and the
//! two conversions disagree about that, since `'0'` is falsy where `"" < "0"` is true.
//!
//! So the branch null cannot be in proves not-null, and the other branch proves *nothing*: the else
//! of `$count > 0` holds `-5` as readily as it holds null. Half of this file is that half.

use expect_test::expect;

use super::{Query, check_view};

const PROVES: &str = r#"
//- /src/Proves.php
<?php
namespace App;

class Proves
{
    public function greater(?int $count): void
    {
        if ($count > 0) {
            $c@greater@ount;
        }
    }

    public function reversed(?int $count): void
    {
        if (0 < $count) {
            $c@reversed@ount;
        }
    }

    public function earlyReturn(?int $count): void
    {
        if ($count <= 0) {
            return;
        }
        $c@after@ount;
    }
}
"#;

const SURVIVES: &str = r#"
//- /src/Survives.php
<?php
namespace App;

class Survives
{
    public function lessThanFive(?int $count): void
    {
        if ($count < 5) {
            $c@less@ount;
        }
    }

    public function atLeastZero(?int $count): void
    {
        if ($count >= 0) {
            $c@atLeast@ount;
        }
    }

    public function elseOfGreater(?int $count): void
    {
        if ($count > 0) {
            return;
        }
        $c@otherwise@ount;
    }

    public function equality(?int $count): void
    {
        if ($count == 3) {
            $c@equal@ount;
        }
    }
}
"#;

#[test]
fn the_branch_null_cannot_be_in_proves_not_null() {
    check_view(
        PROVES,
        &[
            Query::variable_type("`> 0`", "greater"),
            Query::variable_type("`0 <` the other way round", "reversed"),
            Query::variable_type("after `<= 0` returned", "after"),
        ],
        expect![[r#"
            `> 0`
            - int
            `0 <` the other way round
            - int
            after `<= 0` returned
            - int
        "#]],
    );
}

/// The half that keeps the rule honest. Each of these is a branch null survives, measured on PHP
/// 8.5.10: `null < 5` is true, `null >= 0` is true, and the else of `$count > 0` is where null went.
#[test]
fn a_branch_null_survives_proves_nothing() {
    check_view(
        SURVIVES,
        &[
            Query::variable_type("`< 5`, which null satisfies", "less"),
            Query::variable_type("`>= 0`, which null satisfies", "atLeast"),
            Query::variable_type("the else of `> 0`", "otherwise"),
            Query::variable_type("equality, which says nothing", "equal"),
        ],
        expect![[r#"
            `< 5`, which null satisfies
            - ?int
            `>= 0`, which null satisfies
            - ?int
            the else of `> 0`
            - ?int
            equality, which says nothing
            - ?int
        "#]],
    );
}

/// The subject has to *be* the compared value. `count($rows) > 0` says something about `$rows`, but
/// what it says is the existing `count(...)`-against-zero rule's to say -- this one must not reach
/// through a call to the argument inside it.
#[test]
fn a_comparison_of_something_derived_from_the_subject_is_not_about_the_subject() {
    check_view(
        r#"
//- /src/Derived.php
<?php
namespace App;

class Derived
{
    public function run(?int $count): void
    {
        if ($count + 1 > 0) {
            $c@derived@ount;
        }
    }
}
"#,
        &[Query::variable_type("`$count + 1 > 0`", "derived")],
        expect![[r#"
            `$count + 1 > 0`
            - ?int
        "#]],
    );
}

/// `false` is the other sentinel, and it compares the same way: as the least bool, so `false > x`
/// is false and `false <= x` true whatever `x` is. `$at > 0` on the `int|false` of `strtotime()`
/// is how a timestamp is guarded, and it was read as a guard about null alone.
#[test]
fn the_same_branch_rules_false_out() {
    check_view(
        r#"
//- /src/Sentinels.php
<?php
namespace App;

class Sentinels
{
    public function greater(int|false $at): void
    {
        if ($at > 99999999999) {
            $a@greater@t;
        }
    }

    public function earlyReturn(int|false $at): void
    {
        if ($at <= 0) {
            return;
        }
        $a@after@t;
    }

    public function lessThanFive(int|false $at): void
    {
        if ($at < 5) {
            $a@less@t;
        }
    }

    public function elseOfGreater(int|false $at): void
    {
        if ($at > 0) {
            return;
        }
        $a@otherwise@t;
    }
}
"#,
        &[
            Query::variable_type("`> 99999999999`", "greater"),
            Query::variable_type("after `<= 0` returned", "after"),
            Query::variable_type("`< 5`, which false satisfies", "less"),
            Query::variable_type("the else of `> 0`", "otherwise"),
        ],
        expect![[r#"
            `> 99999999999`
            - int
            after `<= 0` returned
            - int
            `< 5`, which false satisfies
            - int|false
            the else of `> 0`
            - int|false
        "#]],
    );
}
