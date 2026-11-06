//! `new $var` when `$var`'s type is a `class-string<T>` rather than a `Foo::class` literal.
//!
//! `phrost-analysis`'s `class_of_an_instance` tests already cover `Foo::class` written in place or
//! held by a local -- read syntactically off the value's own path, because the magic constant is a
//! string at runtime and no type survives it. This file is the other half: a
//! `/** @var class-string<T> */` or a `@param class-string<T>` names the class through the type
//! system instead, and lowering used to fold the whole class-string family down to a bare `string`,
//! discarding `T` before anything downstream could read it.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_locally_annotated_class_string_names_its_class_at_new() {
    check_view(
        r#"
//- /app.php
<?php
class Rule
{
    public function setName(string $name): void {}
}

function make(mixed $className): void
{
    /** @var class-string<Rule> */
    $className = (string) $className;

    $rule = new $className();
    $r@rule@ule;
}
"#,
        &[Query::variable_class(
            "the class the string named",
            "rule",
            "rule",
        )],
        expect![[r#"
            the class the string named
            - Rule
        "#]],
    );
}

/// A guard between the annotation and the `new` does not cost the class-string its argument.
///
/// This used to declare no `class_exists` of its own, so nothing in it narrowed at all -- and it
/// therefore pinned the gap rather than the rule. Every guard shape lost `T`: `is_string`,
/// `=== NULL` and the `class_exists` written here, because `class_a_path_names` walked roots by
/// shape and had an arm for a local and for an annotation and none for a guard. The tag is
/// declared in the fixture because a fixture has no stubs, which is exactly what hid it.
#[test]
fn a_guard_between_the_annotation_and_the_new_keeps_the_class_string_argument() {
    check_view(
        r#"
//- /app.php
<?php
class Rule
{
    public function setName(string $name): void {}
}

/**
 * As PHP's own stubs declare it. Written here because a fixture has no standard library, and
 * without it nothing in this file narrows -- which is what made the earlier version of this test
 * pass while proving nothing.
 *
 * @assert-if-true =class-string $class
 */
function class_exists(string $class, bool $autoload = true): bool {}

class Factory
{
    public function create(array $ruleNode): void
    {
        $fileName = '';
        $className = $ruleNode['class'] ?? ($fileName === '' ? '' : 'x');
        if ((!$className instanceof \Stringable) && !is_string($className)) {
            throw new \RuntimeException('Invalid class');
        }

        /** @var class-string<Rule> */
        $className = (string) $className;

        if (!is_readable($fileName)) {
            $fileName = strtr($className, '\\', '/') . '.php';
        }

        if (!class_exists($className)) {
            throw new \RuntimeException('nope');
        }

        $rule = new $className();
        $r@rule@ule;
    }
}
"#,
        &[Query::variable_class(
            "still class-string<Rule>",
            "rule",
            "rule",
        )],
        expect![[r#"
            still class-string<Rule>
            - Rule
        "#]],
    );
}

#[test]
fn a_bare_var_string_names_no_class_at_new() {
    // The regression this guards: widening the rule to every annotated local rather than to the
    // one shape that actually names a class. A plain `string` still answers nothing.
    check_view(
        r#"
//- /app.php
<?php
function make(mixed $className): void
{
    /** @var string */
    $className = (string) $className;

    $rule = new $className();
    $r@rule@ule;
}
"#,
        &[Query::variable_class(
            "a plain string names nothing",
            "rule",
            "rule",
        )],
        expect![[r#"
            a plain string names nothing
            - (type not known here)
        "#]],
    );
}

#[test]
fn a_parameter_typed_class_string_names_its_class_at_new_too() {
    // `refined_type_ref` already prefers a docblock that "argues about" a native hint -- the same
    // rule `array<T>` beside a native `array` uses -- so a `class-string<T>` docblock beside a
    // native `string` parameter reaches `new $var` through the very same path as a local
    // annotation, with no separate wiring for parameters.
    check_view(
        r#"
//- /app.php
<?php
class Rule
{
    public function setName(string $name): void {}
}

class Factory
{
    /** @param class-string<Rule> $class */
    public function make(string $class): void
    {
        $rule = new $class();
        $r@rule@ule;
    }
}
"#,
        &[Query::variable_class(
            "the parameter's class-string names it",
            "rule",
            "rule",
        )],
        expect![[r#"
            the parameter's class-string names it
            - Rule
        "#]],
    );
}

/// A string literal, which is how a class was named before `::class` -- and, in either quote, with
/// its separator doubled the way PHP escapes it.
#[test]
fn a_string_literal_names_its_class_at_new() {
    check_view(
        r#"
//- /src/Rule.php
<?php
namespace App;

class Rule {}

//- /app.php
<?php
function make(): void
{
    $plain = 'App\Rule';
    $escaped = '\\App\\Rule';
    $named = \App\Rule::class;
    $one = new $plain();
    $two = new $escaped();
    $control = new $named();
    $o@one@ne;
    $t@two@wo;
    $c@control@ontrol;
}
"#,
        &[
            Query::variable_class("written once", "one", "one"),
            Query::variable_class("separator escaped", "two", "two"),
            Query::variable_class("the constant, for comparison", "control", "control"),
        ],
        expect![[r#"
            written once
            - App\Rule
            separator escaped
            - App\Rule
            the constant, for comparison
            - App\Rule
        "#]],
    );
}
