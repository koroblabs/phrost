//! Which scope a name belongs to.
//!
//! The capture rules are the part worth testing hard: they are the difference between "not known"
//! and a confident wrong class. A closure sees only its parameters and what its `use` clause names;
//! an arrow function captures by value automatically and the search continues outward. Getting that
//! backwards describes a variable that is not there.

use expect_test::expect;

use super::check;

#[test]
fn a_methods_parameter() {
    check(
        r#"<?php
class Repo
{
    public function run(Repo $other): void
    {
        $ot$0her;
    }
}
"#,
        "other",
        expect!["Repo"],
    );
}

#[test]
fn a_free_functions_parameter() {
    check(
        r#"<?php
function run(Repo $repo): void
{
    $re$0po;
}
"#,
        "repo",
        expect!["Repo"],
    );
}

#[test]
fn a_closure_parameter_shadows_the_outer_one() {
    check(
        r#"<?php
class Repo
{
    public function run(Alpha $item): void
    {
        $callback = function (Beta $item) {
            $it$0em;
        };
    }
}
"#,
        "item",
        expect!["Beta"],
    );
}

#[test]
fn a_closure_does_not_see_what_it_did_not_capture() {
    // Undefined inside the closure. Naming the outer variable's class would describe something that
    // is not there, and it would look right because the name matches.
    check(
        r#"<?php
class Repo
{
    public function run(Alpha $outer): void
    {
        $callback = function () {
            $ou$0ter;
        };
    }
}
"#,
        "outer",
        expect!["<not declared here>"],
    );
}

#[test]
fn a_closure_sees_what_its_use_clause_names() {
    check(
        r#"<?php
class Repo
{
    public function run(Alpha $outer): void
    {
        $callback = function () use ($outer) {
            $ou$0ter;
        };
    }
}
"#,
        "outer",
        expect!["Alpha"],
    );
}

#[test]
fn an_arrow_function_captures_without_being_asked() {
    check(
        r#"<?php
class Repo
{
    public function run(Alpha $outer): void
    {
        $arrow = fn () => $ou$0ter;
    }
}
"#,
        "outer",
        expect!["Alpha"],
    );
}

#[test]
fn nested_closures_are_walked_from_the_inside_out() {
    check(
        r#"<?php
class Repo
{
    public function run(Alpha $value): void
    {
        $outer = function () use ($value) {
            $inner = function () use ($value) {
                $va$0lue;
            };
        };
    }
}
"#,
        "value",
        expect!["Alpha"],
    );
}

#[test]
fn a_capture_that_stops_partway_out_stops_the_search() {
    // The inner closure captures it, the outer one does not. Reaching the method's parameter anyway
    // would skip a scope where the name is undefined.
    check(
        r#"<?php
class Repo
{
    public function run(Alpha $value): void
    {
        $outer = function () {
            $inner = function () use ($value) {
                $va$0lue;
            };
        };
    }
}
"#,
        "value",
        expect!["<not declared here>"],
    );
}

#[test]
fn an_untyped_parameter_declares_the_name_without_giving_it_a_type() {
    // Declared here, so the search stops: an outer parameter of the same name is a different
    // variable, and its type would be the wrong answer rather than a missing one.
    check(
        r#"<?php
class Repo
{
    public function run(Alpha $item): void
    {
        $callback = function ($item) {
            $it$0em;
        };
    }
}
"#,
        "item",
        expect!["<not declared here>"],
    );
}

#[test]
fn an_offset_outside_every_body_declares_nothing() {
    // On the *type hint*, which is the half of a signature that binds nothing: no local is written
    // there, so there is no scope to answer about. This used to be true of the parameter's own name
    // beside it as well, and that was the bug -- see below.
    check(
        r#"<?php
class Repo
{
    public function run(Re$0po $other): void
    {
    }
}
"#,
        "other",
        expect!["<not declared here>"],
    );
}

#[test]
fn a_parameter_declares_itself_at_its_own_name() {
    // The name in the signature is the local's binding site, and the only position a person renames
    // a parameter from. Answering nothing here left rename, document highlight, references and
    // goto-type-definition all dead at it, and alive one word to the right.
    check(
        r#"<?php
class Repo
{
    public function run(Repo $ot$0her): void
    {
    }
}
"#,
        "other",
        expect!["Repo"],
    );
}

#[test]
fn a_variable_nothing_declares() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $mis$0sing;
    }
}
"#,
        "missing",
        expect!["<not declared here>"],
    );
}
