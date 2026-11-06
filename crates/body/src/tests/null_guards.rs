//! What a null test proves, and about which value.
//!
//! Separate from the `instanceof` tests because the claim is a different shape: `instanceof` names a
//! class, and these name nothing -- they say which of the things the value could be are now ruled
//! out, leaving the layer that knows the type to turn that into an answer. Which is also why the
//! subject travels with them.

use expect_test::expect;

use super::check;

#[test]
fn an_early_return_on_null_rules_it_out_below() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $value): void
    {
        if ($value === null) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["not null $value"],
    );
}

#[test]
fn a_negated_comparison_rules_it_out_inside() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $value): void
    {
        if ($value !== null) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["not null $value"],
    );
}

#[test]
fn is_null_is_the_same_claim_as_a_comparison() {
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $value): void
    {
        if (is_null($value)) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["null, exactly, $value"],
    );
}

#[test]
fn the_guarded_side_of_a_comparison_is_the_one_that_holds() {
    // `$a === null || $a->x()` reaches the right operand only when the left was false, so the value
    // is *not* null there. Getting the polarity wrong here is the difference between a silent
    // checker and one reporting every guarded access in a project.
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $value): void
    {
        $ok = $value === null || $val$0ue;
    }
}
"#,
        "value",
        expect!["not null $value"],
    );
}

#[test]
fn a_write_after_the_guard_makes_the_guard_stale() {
    // A form class puts `if (isset($this->x)) { return; }` at the top of a method and assigns below
    // it. Reading the guard past the assignment says the value is null for the rest of the method.
    check(
        r#"<?php
class Repo
{
    public function run(?Alpha $value): void
    {
        if ($value === null) {
            return;
        }
        $value = $this->reload();
        $val$0ue;
    }
}
"#,
        "value",
        expect!["$this->reload()"],
    );
}

#[test]
fn a_loop_variable_beats_an_assignment_written_above_it() {
    // Ranked by position and not by which construct it was: the loop is what the body of the loop
    // sees, and reading the assignment there gave every use inside the wrong type.
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $one = $this->selected();
        foreach ($this->all() as $one) {
            $o$0ne;
        }
    }
}
"#,
        "one",
        expect!["$this->all()[element]"],
    );
}

#[test]
fn an_assignment_tested_as_a_condition_narrows_what_it_assigned() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        if ($one = $this->find()) {
            $o$0ne;
        }
    }
}
"#,
        "one",
        expect!["not null $this->find()"],
    );
}

#[test]
fn a_guard_inside_a_for_loop_is_a_guard() {
    // A `for` loop was unmodelled, and an unmodelled statement keeps its expressions but not its
    // statements -- so every guard, `foreach` and `catch` written inside one used to be invisible.
    check(
        r#"<?php
class Repo
{
    public function run(int $count): void
    {
        for ($i = 0; $i < $count; $i++) {
            $one = $this->find();
            if (!$one) {
                continue;
            }
            $o$0ne;
        }
    }
}
"#,
        "one",
        expect!["not null $one"],
    );
}
