//! A branch ending in a call declared `: never` drops its writes, wherever the call is written.
//!
//! `if ($c) { $x = new Bar(); abort(); }` leaves `$x` what it was before the `if`: control cannot
//! come back out of that branch, so the write in it is never in effect after it. Two things are
//! pinned here that each broke on their own. The body facts kept only the tree's half of "does this
//! branch leave", and the tree cannot see a `: never` declaration -- so the reader's oracle has to
//! be asked at lookup, and a call that *looks* ordinary has to stay ordinary. And a branch was read
//! by its last statement, so `abort(); $dead = 1;` counted as falling through, which is exactly how
//! a guard clause is written once somebody forgets to delete the line after it.

use expect_test::expect;

use super::{Query, check_view};

/// Two classes and two ways to leave: a function and a method, both declared `never`.
const CLASSES: &str = r#"
//- /src/classes.php
<?php
class Foo
{
    public function fooMethod(): void {}
}
class Bar
{
    public function barMethod(): void {}
}
function abort_now(): never
{
    throw new \RuntimeException();
}
function log_it(): void {}
class Guarded
{
    protected function fail(): never
    {
        exit(1);
    }
}
"#;

#[test]
fn a_write_before_a_never_returning_call_is_not_in_effect_after_the_branch() {
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function test(): void
{{
    $obj = new Foo();
    if (rand(0, 1)) {{
        $obj = new Bar();
        abort_now();
    }}
    $obj@after@;
}}
"#
        ),
        &[Query::variable_class("after the if", "after", "obj")],
        expect![[r#"
            after the if
            - Foo
        "#]],
    );
}

#[test]
fn a_call_that_returns_leaves_the_branch_joining_as_before() {
    // The control: the same shape with an ordinary call, so it is the declaration and not the
    // syntax that decides.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function test(): void
{{
    $obj = new Foo();
    if (rand(0, 1)) {{
        $obj = new Bar();
        log_it();
    }}
    $obj@after@;
}}
"#
        ),
        &[Query::variable_type("after the if", "after")],
        expect![[r#"
            after the if
            - Bar|Foo
        "#]],
    );
}

#[test]
fn dead_code_after_the_call_does_not_make_the_branch_fall_through() {
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function test(Foo|Bar $x): void
{{
    if (!$x instanceof Foo) {{
        abort_now();
        $unreachable = 1;
    }}
    $x@after@;
}}
"#
        ),
        &[Query::variable_class("after the guard", "after", "x")],
        expect![[r#"
            after the guard
            - Foo
        "#]],
    );
}

#[test]
fn a_method_declared_never_counts_too() {
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
class Runner extends Guarded
{{
    public function run(): void
    {{
        $obj = new Foo();
        if (rand(0, 1)) {{
            $obj = new Bar();
            $this->fail();
        }}
        $obj@after@;
    }}
}}
"#
        ),
        &[Query::variable_class("after the if", "after", "obj")],
        expect![[r#"
            after the if
            - Foo
        "#]],
    );
}
