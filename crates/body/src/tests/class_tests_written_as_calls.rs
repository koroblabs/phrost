//! `is_a($x, Foo::class)` and `is_subclass_of($x, Foo::class)` -- `instanceof` written as a call.
//!
//! The same claim through a different spelling, which is the form this codebase keeps meeting: a
//! rule taught to the operator and not to its functional twin answers on one arm and is silent on
//! the other. What the call adds is a class that need not be spelled in the source, so it is what a
//! plugin registry or a `::class` held in a constant is tested with.

use expect_test::expect;

use super::check;

#[test]
fn is_a_narrows_inside_the_branch() {
    check(
        r#"<?php
class Base {}
class Derived extends Base {}
class Repo
{
    public function run(Base $value): void
    {
        if (is_a($value, Derived::class)) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["instanceof Derived $value"],
    );
}

#[test]
fn is_a_rules_the_class_out_on_the_branch_not_taken() {
    check(
        r#"<?php
class Base {}
class Derived extends Base {}
class Repo
{
    public function run(Base $value): void
    {
        if (is_a($value, Derived::class)) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["not instanceof Derived $value"],
    );
}

/// The subclass test proves the named class inside the branch, because a subclass is one -- but
/// only where a class *name* cannot arrive, which for this function has to be written out.
#[test]
fn is_subclass_of_narrows_to_the_class_it_names() {
    check(
        r#"<?php
class Base {}
class Repo
{
    public function run(object $value): void
    {
        if (is_subclass_of($value, Base::class, false)) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["instanceof Base $value"],
    );
}

/// And proves nothing on the other branch, because it is false for the class itself.
#[test]
fn is_subclass_of_rules_nothing_out_when_it_is_false() {
    check(
        r#"<?php
class Base {}
class Repo
{
    public function run(object $value): void
    {
        if (is_subclass_of($value, Base::class, false)) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["object"],
    );
}

/// `is_a($name, Foo::class, true)` accepts a class *name*, so what it proves is about a string.
#[test]
fn a_third_argument_makes_the_subject_a_name_and_is_refused() {
    check(
        r#"<?php
class Base {}
class Repo
{
    public function run(object $value): void
    {
        if (is_a($value, Base::class, true)) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["object"],
    );
}

/// A class the call names through a value resolves to no declaration, and inventing one is the
/// fallback this project refuses.
#[test]
fn a_class_held_in_a_variable_names_nothing_to_narrow_to() {
    check(
        r#"<?php
class Repo
{
    public function run(object $value, string $class): void
    {
        if (is_a($value, $class)) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["object"],
    );
}

/// `is_subclass_of` admits a class name unless told otherwise, and checking a `class-string` before
/// `$className::make()` is what it is mostly written for. Three correct lines on one real checkout
/// were reported by reading this as a claim about the value.
#[test]
fn is_subclass_of_says_nothing_about_a_value_that_may_be_a_class_name() {
    check(
        r#"<?php
class Base {}
class Repo
{
    public function run(object $value): void
    {
        if (is_subclass_of($value, Base::class)) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["object"],
    );
}
