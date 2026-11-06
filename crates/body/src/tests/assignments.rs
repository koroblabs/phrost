//! What the most recent assignment before the cursor says a name holds.
//!
//! Flow-insensitive on purpose, and ordered: the last assignment *written* before the offset wins,
//! whether or not the branch it sits in runs.

use expect_test::expect;

use super::check;

#[test]
fn an_assignment_from_a_constructor() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $repo = new Repo();
        $re$0po;
    }
}
"#,
        "repo",
        expect!["new Repo"],
    );
}

#[test]
fn the_last_assignment_before_the_cursor_wins() {
    // Most recent binding, textually. An earlier one describes a value that has been replaced.
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $thing = new Alpha();
        $thing = new Beta();
        $th$0ing;
        $thing = new Gamma();
    }
}
"#,
        "thing",
        expect!["new Beta"],
    );
}

#[test]
fn an_assignment_beats_the_parameter_it_overwrote() {
    check(
        r#"<?php
class Repo
{
    public function run(Alpha $thing): void
    {
        $thing = new Beta();
        $th$0ing;
    }
}
"#,
        "thing",
        expect!["new Beta"],
    );
}

#[test]
fn a_parameter_still_answers_before_the_assignment_that_replaces_it() {
    check(
        r#"<?php
class Repo
{
    public function run(Alpha $thing): void
    {
        $th$0ing;
        $thing = new Beta();
    }
}
"#,
        "thing",
        expect!["Alpha"],
    );
}

#[test]
fn an_assignment_from_a_property_of_this() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $repo = $this->inner;
        $re$0po;
    }
}
"#,
        "repo",
        expect!["$this->$inner"],
    );
}

#[test]
fn an_assignment_from_a_chain_keeps_every_step() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $name = $this->repo->find(1)->label;
        $na$0me;
    }
}
"#,
        "name",
        expect!["$this->$repo->find(@0 '1')->$label"],
    );
}

#[test]
fn an_assignment_from_a_static_call() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $made = Factory::make();
        $ma$0de;
    }
}
"#,
        "made",
        expect!["Factory->make()"],
    );
}

#[test]
fn an_assignment_from_another_local_is_left_for_the_caller_to_follow() {
    // Not followed here: the other local may be bound in an enclosing scope, and the caller already
    // knows how to ask about one.
    check(
        r#"<?php
class Repo
{
    public function run(Alpha $source): void
    {
        $alias = $source;
        $al$0ias;
    }
}
"#,
        "alias",
        expect!["$source"],
    );
}

#[test]
fn a_chained_assignment_takes_the_value_both_names_got() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $a = $b = new Repo();
        $$0a;
    }
}
"#,
        "a",
        expect!["new Repo"],
    );
}

#[test]
fn an_operator_that_decides_its_own_result_states_it() {
    // `.` builds a string out of whatever it was given, so the operands need not be known for the
    // result to be. The comparison and bitwise operators are the same shape.
    check(
        r#"<?php
class Repo
{
    public function run($items): void
    {
        $value = $items . 'x';
        $va$0lue;
    }
}
"#,
        "value",
        expect!["(non-falsy-string)"],
    );
}

#[test]
fn an_arithmetic_operator_carries_both_operands() {
    // What `*` produces is decided by what it was given, so the path keeps both sides for whoever
    // can resolve them. Two ints make an int; a parameter with no type at all makes nothing.
    check(
        r#"<?php
class Repo
{
    public function run($items): void
    {
        $value = $items * 2;
        $va$0lue;
    }
}
"#,
        "value",
        expect!["$items * (int)"],
    );
}

#[test]
fn a_prefix_step_is_its_own_root() {
    // `++$count` is `$count + 1` only for a number: PHP leaves a bool alone and counts a string like a
    // column name. Carried apart from the arithmetic, so the coercions `+` makes are not claimed for
    // a step that does not make them.
    check(
        r#"<?php
class Repo
{
    public function run($count): void
    {
        $next = ++$count;
        $ne$0xt;
    }
}
"#,
        "next",
        expect!["++$count"],
    );
}

#[test]
fn a_division_carries_the_operator_that_makes_it_one() {
    // `/` is the one arithmetic operator whose result PHP does not settle by the operand types
    // alone: dividing two ints gives an int when it divides evenly and a float when it does not.
    // This layer cannot say which -- it does not resolve anything -- so what it has to carry is the
    // operator, and the layer that resolves the operands answers `int|float`.
    check(
        r#"<?php
class Repo
{
    public function run($items): void
    {
        $value = $items / 2;
        $va$0lue;
    }
}
"#,
        "value",
        expect!["$items / (int)"],
    );
}

#[test]
fn an_assignment_inside_a_closure_belongs_to_the_closure() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $callback = function () {
            $inner = new Beta();
        };
        $in$0ner;
    }
}
"#,
        "inner",
        expect!["<not declared here>"],
    );
}

#[test]
fn an_assignment_in_a_branch_is_taken_whether_or_not_the_branch_runs() {
    // Not as a replacement, though: the branch may not have run, so the value is what it wrote
    // *or* what the name held before the `if`. Here nothing else ever wrote `$thing`, so the
    // lookup before the branch finds nothing and the write is the whole answer -- which is what
    // keeps `$thing->` offering Alpha's members.
    check(
        r#"<?php
class Repo
{
    public function run($condition): void
    {
        if ($condition) {
            $thing = new Alpha();
        }
        $th$0ing;
    }
}
"#,
        "thing",
        expect!["new Alpha or $thing before"],
    );
}

#[test]
fn an_assignment_from_a_cast_takes_the_type_the_cast_states() {
    // The plainest statement of type a PHP file can make, and it used to give nothing: `(int)` is
    // unmodelled as an operator, so the whole assignment was.
    check(
        r#"<?php
class Service
{
    public function run(): void
    {
        $uid = (int) $this->id();
        $ui$0d;
    }
}
"#,
        "uid",
        expect!["(int)"],
    );
}

#[test]
fn every_spelling_of_a_cast_means_the_same_type() {
    // PHP spells three of these two or three ways, and they are the same claim.
    check(
        r#"<?php
class Service
{
    public function run($value): void
    {
        $text = (string) $value;
        $tex$0t;
    }
}
"#,
        "text",
        expect!["(string) of $value"],
    );
    check(
        r#"<?php
class Service
{
    public function run($value): void
    {
        $ratio = (double) $value;
        $rati$0o;
    }
}
"#,
        "ratio",
        expect!["(float)"],
    );
}
