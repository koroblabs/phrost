//! A flag proves what its condition proved about the value the subject held when it was written.
//!
//! `$ok = $item !== null;` remembers a fact about one `$item`, and the claim is resolved where the
//! flag is tested -- so a write to `$item` in between used to hand the new value a non-null nobody
//! checked, and the `possibly-null` that should have followed went silent. PHPStan reports every
//! subject here that is not narrowed, and none that is.
//!
//! The other half is which write of the flag is the one tested. The latest by position is not it
//! when an earlier one, in another branch, reaches the guard as well.

use expect_test::expect;

use super::check;

#[test]
fn a_subject_replaced_after_the_flag_is_not_narrowed_by_it() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $item, ?Alpha $next): void
    {
        $ok = $item !== null;
        $item = $next;
        if ($ok) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect![[r#"$next"#]],
    );
}

#[test]
fn a_subject_that_may_have_been_replaced_is_not_narrowed_either() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $item, ?Alpha $next, bool $c): void
    {
        $ok = $item !== null;
        if ($c) {
            $item = $next;
        }
        if ($ok) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect![[r#"$next or $item before"#]],
    );
}

#[test]
fn a_subject_replaced_after_a_flag_two_steps_away_is_not_narrowed() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $item, ?Alpha $next): void
    {
        $ok = $item !== null;
        $again = $ok;
        $item = $next;
        if ($again) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect![[r#"$next"#]],
    );
}

/// The control: nothing written between, so the flag is the condition.
#[test]
fn a_subject_left_alone_is_narrowed_by_the_flag() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $item): void
    {
        $ok = $item !== null;
        if ($ok) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect![[r#"not null $item"#]],
    );
}

/// A write in a branch that returns never reaches the test, so the flag still describes the value.
#[test]
fn a_replacement_in_a_branch_that_leaves_does_not_reach_the_test() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $item, ?Alpha $next, bool $c): void
    {
        $ok = $item !== null;
        if ($c) {
            $item = $next;
            return;
        }
        if ($ok) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect![[r#"not null $item"#]],
    );
}

/// The write the flag was assigned from is the one the claim was read off, so writing the subject
/// *inside* it is what the claim is about.
#[test]
fn a_subject_written_inside_the_flag_is_the_value_it_describes() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $next): void
    {
        $ok = ($item = $next) !== null;
        if ($ok) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect![[r#"not null $next"#]],
    );
}

#[test]
fn a_flag_written_on_two_branches_proves_what_neither_alone_does() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $item, bool $c, bool $x): void
    {
        if ($c) {
            $ok = $x;
        } else {
            $ok = $item !== null;
        }
        if ($ok) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect!["?Alpha"],
    );
}

/// The way a flag is written conditionally: a default that can never test true, and the condition
/// in a branch. True only where the branch ran.
#[test]
fn a_flag_defaulted_to_false_proves_the_condition_it_may_have_been_set_from() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $item, bool $c): void
    {
        $ok = false;
        if ($c) {
            $ok = $item !== null;
        }
        if ($ok) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect![[r#"not null $item"#]],
    );
}

/// The same flag tested false: the default is false too, so the branch need not have run.
#[test]
fn a_flag_defaulted_to_false_proves_nothing_when_it_tests_false() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $item, bool $c): void
    {
        $missing = false;
        if ($c) {
            $missing = $item === null;
        }
        if (!$missing) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect!["?Alpha"],
    );
}

/// A compound write leaves the flag holding something no condition was read from.
#[test]
fn a_flag_widened_by_a_compound_write_proves_nothing() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $item, int $x): void
    {
        $ok = $item !== null;
        $ok |= $x;
        if ($ok) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect!["?Alpha"],
    );
}
