//! Resolving `PathRoot::AssertedArgument` end to end: the callee's declaration is looked up, its
//! tag is read, and the tag's own polarity is checked against the branch's before it is trusted --
//! finding the declaration a call like `Validator::isUser($x)` or `$validator->isUser($x)` names,
//! since neither call is made on `$x` itself, the way its receiver-form sibling is.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_static_call_resolves_the_class_its_tag_names() {
    check_view(
        r#"
//- /src/app.php
<?php
class User
{
    public function getName(): string {}
}

class Validator
{
    /**
     * @phpstan-assert-if-true User $value
     */
    public static function isUser(mixed $value): bool {}
}

function test(mixed $input): void
{
    if (Validator::isUser($input)) {
        $inp@here@ut;
    }
}
"#,
        &[Query::variable_class("narrowed", "here", "input")],
        expect![[r#"
            narrowed
            - User
        "#]],
    );
}

#[test]
fn an_instance_call_resolves_the_class_its_tag_names_too() {
    check_view(
        r#"
//- /src/app.php
<?php
class User
{
    public function getName(): string {}
}

class Validator
{
    /**
     * @phpstan-assert-if-true User $value
     */
    public function isUser(mixed $value): bool {}
}

function test(Validator $validator, mixed $input): void
{
    if ($validator->isUser($input)) {
        $inp@here@ut;
    }
}
"#,
        &[Query::variable_class("narrowed", "here", "input")],
        expect![[r#"
            narrowed
            - User
        "#]],
    );
}

#[test]
fn a_leaving_guard_resolves_the_class_for_the_rest_of_the_method() {
    check_view(
        r#"
//- /src/app.php
<?php
class Email
{
    public function send(): void {}
}

class Guard
{
    /**
     * @phpstan-assert-if-false Email $value
     */
    public static function isInvalid(mixed $value): bool {}
}

function test(mixed $input): void
{
    if (Guard::isInvalid($input)) {
        return;
    }
    $inp@here@ut;
}
"#,
        &[Query::variable_class("narrowed", "here", "input")],
        expect![[r#"
            narrowed
            - Email
        "#]],
    );
}

/// `!Guard::isInvalid($input)` inside the branch it guards proves the *branch's* polarity is
/// false for the call itself -- "not invalid" -- which is exactly what the `-if-false` tag is
/// about. Getting the negation backwards here would resolve nothing, or resolve the wrong class
/// on a tag that named one on each branch.
#[test]
fn a_negated_call_matches_the_if_false_tag_it_flips_into() {
    check_view(
        r#"
//- /src/app.php
<?php
class Email
{
    public function send(): void {}
}

class Guard
{
    /**
     * @phpstan-assert-if-false Email $value
     */
    public static function isInvalid(mixed $value): bool {}
}

function test(mixed $input): void
{
    if (!Guard::isInvalid($input)) {
        $inp@here@ut;
    }
}
"#,
        &[Query::variable_class("narrowed", "here", "input")],
        expect![[r#"
            narrowed
            - Email
        "#]],
    );
}

/// The tag says what *one* return value proves; the opposite branch proves nothing, the same rule
/// the receiver form already follows. Conflating the branch's polarity with the tag's would
/// narrow here too, which is exactly the bug a `!` in front of the call would have exposed.
#[test]
fn a_tag_for_the_opposite_branch_narrows_nothing() {
    check_view(
        r#"
//- /src/app.php
<?php
class User
{
    public function getName(): string {}
}

class Validator
{
    /**
     * @phpstan-assert-if-true User $value
     */
    public static function isUser(mixed $value): bool {}
}

function test(mixed $input): void
{
    if (Validator::isUser($input)) {
    } else {
        $inp@here@ut;
    }
}
"#,
        &[Query::variable_class("narrowed", "here", "input")],
        expect![[r#"
            narrowed
            - (type not known here)
        "#]],
    );
}

/// A named argument fills its parameter by name, not by the position it happens to sit at in the
/// call -- `value: $input` is the call's *only* argument, at position 0, but the tag names the
/// parameter written at position 1. Matching by position instead would miss it.
#[test]
fn a_named_argument_matches_its_parameter_rather_than_its_position() {
    check_view(
        r#"
//- /src/app.php
<?php
class User
{
    public function getName(): string {}
}

class Validator
{
    /**
     * @phpstan-assert-if-true User $value
     */
    public static function isUser(mixed $other, mixed $value): bool {}
}

function test(mixed $input): void
{
    if (Validator::isUser(value: $input)) {
        $inp@here@ut;
    }
}
"#,
        &[Query::variable_class("narrowed", "here", "input")],
        expect![[r#"
            narrowed
            - User
        "#]],
    );
}

/// PHPUnit's `assertIsArray`, `assertIsString` and the rest write a *kind* rather than a class --
/// `@phpstan-assert array $actual` -- and the value has to be read the way `is_array($x)` reads it:
/// the alternatives of that kind survive, null and the rest do not. This fell through to the class
/// lookup, found no class, and left the value as it was, so `assertIsArray($group)` narrowed nothing
/// and `assertArrayNotHasKey('k', $group)` on the next line reported a null.
#[test]
fn a_kind_assertion_removes_the_null_and_the_other_kinds() {
    check_view(
        r#"
//- /src/app.php
<?php
class Registry
{
    /** @return array<string, mixed>|null */
    public function get(string $name): ?array {}

    public function label(): string|int|null {}
}

abstract class Assert
{
    /**
     * @phpstan-assert array $actual
     */
    public static function assertIsArray(mixed $actual, string $message = ''): void {}

    /**
     * @phpstan-assert string $actual
     */
    public static function assertIsString(mixed $actual): void {}

    /**
     * @phpstan-assert !array $actual
     */
    public static function assertIsNotArray(mixed $actual): void {}
}

class Test extends Assert
{
    public function run(Registry $registry, mixed $anything, array|string $either): void
    {
        $group = $registry->get('x');
        $this->assertIsArray($group);
        $gr@array@oup;

        $label = $registry->label();
        self::assertIsString($label);
        $la@string@bel;

        $this->assertIsArray($anything);
        $anyt@mixed@hing;

        $this->assertIsNotArray($either);
        $eit@negated@her;
    }
}
"#,
        &[
            Query::variable_type("a nullable array asserted to be an array", "array"),
            Query::variable_type("a scalar union asserted to be a string", "string"),
            Query::variable_type("a mixed asserted to be an array", "mixed"),
            Query::variable_type("a union asserted not to be an array", "negated"),
        ],
        expect![[r#"
            a nullable array asserted to be an array
            - array<string, mixed>
            a scalar union asserted to be a string
            - string
            a mixed asserted to be an array
            - array
            a union asserted not to be an array
            - string
        "#]],
    );
}

/// A plain function written directly in the guard, whose docblock names one of its arguments.
///
/// WordPress writes `@phpstan-assert-if-true WP_Error $thing` on `is_wp_error` in its own
/// `load.php`, and until this was read nothing in the project could see it: the arm matching
/// `ExprData::FunctionCall` is the one for PHP's kind tests and answered `None` for a name that is
/// not one. Measured at 150 findings on that checkout -- 46 `possibly-false-receiver`, 32
/// `possibly-null-return`, 22 `possibly-null-receiver`, 21 `incompatible-argument` and the rest,
/// all under an `is_wp_error` about the same variable.
///
/// The reason it is gated on `AssertsArguments` rather than admitted for every call is that this
/// root resolves `of` at the *guard*: one built for a callee that carries no assertion answers with
/// what the subject held before the branch rather than what it holds at the use. Building one per
/// `if (f($x))` in a project loses precision nothing asked it to -- measured as the out-parameter
/// suite reporting "narrowed by a guard" for five positions an out-parameter had written, type
/// unchanged. A name set the index already holds rules that out before anything is resolved.
///
/// The kind tests keep their own answer: a name `type_predicate` recognises never reaches the tag,
/// because "this value is a string" says more than any assertion could.
#[test]
fn a_plain_function_in_the_guard_names_the_class_it_asserts() {
    check_view(
        r#"
//- /src/app.php
<?php
class WP_Error
{
    public function get_error_message(): string {}
}

/**
 * @phpstan-assert-if-true WP_Error $thing
 */
function is_wp_error(mixed $thing): bool {}

function test(mixed $result): void
{
    if (is_wp_error($result)) {
        $resu@here@lt;
    }
}
"#,
        &[Query::variable_class("narrowed", "here", "result")],
        expect![[r#"
            narrowed
            - WP_Error
        "#]],
    );
}

/// A guard calling a function that promises nothing leaves the value exactly as it was.
///
/// The other half of the gate, and the half that fails silently: an unguarded arm builds a root for
/// every `if (f($x))`, and a root resolves its subject at the guard. So the value does not become
/// wrong, it becomes *stale* -- and the binding reads "narrowed by a guard" for a position no guard
/// narrowed, which is a sentence about somebody's code rather than a type. Here `looksOk` carries no
/// tag, so `$result` keeps the class the line above gave it.
#[test]
fn a_guard_on_a_function_that_promises_nothing_leaves_the_value_alone() {
    check_view(
        r#"
//- /src/app.php
<?php
class Payload
{
    public function body(): string {}
}

function looksOk(mixed $thing): bool {}

function test(): void
{
    $result = new Payload();
    if (looksOk($result)) {
        $resu@here@lt;
    }
}
"#,
        &[Query::variable_class("not narrowed", "here", "result")],
        expect![[r#"
            not narrowed
            - Payload
        "#]],
    );
}
