//! A write is only the value of a name where control had to go through it.
//!
//! The rule these pin, in one sentence: a write reaches a position **unconditionally** when every
//! branch holding the write also holds the position, **not at all** when the position is in a
//! sibling branch of the same construct or the write's branch never comes back, and **conditionally**
//! otherwise -- where it joins what the name already held instead of replacing it.
//!
//! Position alone is not the rule and never was. `if ($c) { $v = trim($v); } else { $v-> }` puts the
//! nearest earlier write on a path the cursor is not on, and reading it there is a wrong answer that
//! every check reading a variable's type then reports on.

use expect_test::expect;

use super::check;

#[test]
fn a_write_in_the_other_branch_of_an_if_is_not_in_effect() {
    // One condition chooses one side, so the then-branch never ran on the way here. The parameter
    // is what is left, which is the whole answer rather than a weaker one.
    check(
        r#"<?php
class Repo
{
    public function run(mixed $v, bool $c): void
    {
        if ($c) {
            $v = trim($v);
        } else {
            $$0v;
        }
    }
}
"#,
        "v",
        expect!["mixed"],
    );
}

#[test]
fn a_write_in_an_earlier_if_may_have_run() {
    // Two separate `if`s are not two branches of one construct, whatever their conditions say --
    // nothing structural rules the first one out. So the write joins rather than replacing, and
    // `string or mixed` is a mixed, which is the honest answer for a value the file does not pin.
    check(
        r#"<?php
class Repo
{
    public function run(mixed $v, bool $c): void
    {
        if ($c) {
            $v = trim($v);
        }
        if (!$c) {
            $$0v;
        }
    }
}
"#,
        "v",
        expect!["trim() or $v before"],
    );
}

#[test]
fn a_write_in_another_match_arm_is_not_in_effect() {
    // The same exclusion as an `if`, written as an expression. A `match` picks one arm.
    check(
        r#"<?php
class Repo
{
    public function run(mixed $v, string $k): void
    {
        match ($k) {
            'a' => $v = trim($v),
            'b' => $$0v,
            default => null,
        };
    }
}
"#,
        "v",
        expect!["mixed"],
    );
}

#[test]
fn a_case_that_broke_does_not_reach_a_later_one() {
    // A `switch` arm reaches the next only by falling into it, so a `break` is what makes two arms
    // exclusive -- and without one they are not. See the sibling test below.
    check(
        r#"<?php
class Repo
{
    public function run(mixed $v, string $k): void
    {
        switch ($k) {
            case 'a':
                $v = trim($v);
                break;

            case 'b':
                $$0v;
                break;
        }
    }
}
"#,
        "v",
        expect!["mixed"],
    );
}

#[test]
fn a_case_that_falls_through_may_reach_the_next() {
    // No `break`, so control runs straight on from one arm into the other and the write is back in
    // play. Joined rather than in effect outright: the subject matching `'b'` enters the second arm
    // directly and skips the first, which is the same "may have run" every other branch has.
    check(
        r#"<?php
class Repo
{
    public function run(mixed $v, string $k): void
    {
        switch ($k) {
            case 'a':
                $v = trim($v);

            case 'b':
                $$0v;
                break;
        }
    }
}
"#,
        "v",
        expect!["trim() or $v before"],
    );
}

#[test]
fn a_write_on_the_way_replaces_what_came_before() {
    // The control for all of the above: nothing branches, so the write is simply in effect. This is
    // the one shape that must keep reporting when a value is passed on somewhere it does not fit.
    check(
        r#"<?php
class Repo
{
    public function run(mixed $v): void
    {
        $v = trim($v);
        $$0v;
    }
}
"#,
        "v",
        expect!["trim()"],
    );
}

#[test]
fn a_write_inside_the_branch_the_position_is_in_replaces_too() {
    // Being in a branch says nothing extra when the position is in the same one.
    check(
        r#"<?php
class Repo
{
    public function run(mixed $v, bool $c): void
    {
        if ($c) {
            $v = trim($v);
            $$0v;
        }
    }
}
"#,
        "v",
        expect!["trim()"],
    );
}

#[test]
fn a_write_in_a_branch_that_returns_never_arrives() {
    // The branch does not come back, so nothing written in it is in effect after it -- the same
    // rule an early-return guard already relies on to narrow the code below it.
    check(
        r#"<?php
class Repo
{
    public function run(mixed $v, bool $c): void
    {
        if ($c) {
            $v = trim($v);
            return;
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["mixed"],
    );
}

#[test]
fn several_branches_that_write_join_as_one_union() {
    // N operands, not a chain of two: each arm is resolved once, beside one lookup for what the
    // name held before the switch. Nesting them would spend a level of the resolution budget per
    // arm, and there are only twelve.
    check(
        r#"<?php
class Repo
{
    public function run(mixed $v, string $k): void
    {
        switch ($k) {
            case 'a':
                $v = trim($v);
                break;

            case 'b':
                $v = 1;
                break;

            case 'c':
                $v = [];
                break;
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["trim() or (int) or (array) or $v before"],
    );
}

#[test]
fn a_write_in_a_loop_body_may_not_have_run() {
    // A loop runs zero times as readily as once, so its body is a branch like any other.
    check(
        r#"<?php
class Repo
{
    public function run(mixed $v, array $rows): void
    {
        foreach ($rows as $row) {
            $v = trim($v);
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["trim() or $v before"],
    );
}

#[test]
fn a_branch_entered_because_the_name_was_null_completes_it() {
    // The commonest reason to write a name in a branch at all. Such a branch is not an alternative
    // to what the name held -- it is the completion of it, and the path that skipped it is exactly
    // the path where the value was not null. Joining the declaration whole would put back the one
    // possibility the author wrote the branch to remove.
    //
    // Rendered "minus falsehood" rather than "minus null" because the guard is `!$v` and not
    // `=== NULL`: truthiness rules `false` out of the skipped path as well, which is what the flag
    // carries. For a `?string` the two come to the same thing; for an `array|false` they do not.
    check(
        r#"<?php
class Repo
{
    public function run(?string $v): void
    {
        if (!$v) {
            $v = 'fallback';
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["(string) or $v before minus falsehood"],
    );
}

#[test]
fn a_branch_entered_because_the_name_was_not_of_a_class_completes_it_too() {
    // The other half of the same idiom, and the commoner one in typed code: normalise the value if
    // it is not already what the code needs. The path that skipped the branch is the path where it
    // was a `Document`, whatever the parameter's own hint says -- and `object` says nothing at all,
    // so joining it whole would leave a value with no members.
    check(
        r#"<?php
class Document
{
    public function run(): void {}
}
class Repo
{
    public function build(object $v): void
    {
        if (!$v instanceof Document) {
            $v = new Document();
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["new Document or $v before as Document"],
    );
}
