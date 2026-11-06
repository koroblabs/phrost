//! A method that does not fit the one it overrides.
//!
//! A fatal error at class *load*, before any of the class runs. Which is also why it can sit in a
//! codebase for years: it fires only when something loads that class.
//!
//! Five exact rules, and no types. PHP's variance rules for parameter and return types need a
//! subtyping relation over unions, intersections, nullables and PHP's coercions -- and half of one
//! reports code that is correct, which is worse than reporting nothing.

use expect_test::expect;

use super::diagnostics::check;

/// Every way a subclass may legally differ. All of it silent.
#[test]
fn a_compatible_override_says_nothing() {
    check(
        r#"
//- /app.php
<?php
class Base
{
    public function same(int $a, int $b): void {}

    public function fine(int $a): void {}

    public static function stat(): void {}

    private function mine(): void {}

    public function variadic(int ...$rest): void {}

    protected function widened(): void {}
}

class Good extends Base
{
    public function same(int $a, int $b): void {}

    public function fine(int $a, int $b = 0): void {}

    public static function stat(): void {}

    private function mine(int $whatever): void {}

    public function variadic(int ...$rest): void {}

    public function widened(): void {}
}
"#,
        expect![""],
    );
}

/// A private method in the parent is not overridden at all -- PHP treats the two as unrelated, so a
/// child may declare whatever it likes under the same name. Its own test because getting it wrong
/// would report a great deal of correct code.
#[test]
fn a_private_parent_method_is_not_an_override() {
    check(
        r#"
//- /app.php
<?php
class Base
{
    private function mine(int $a, int $b): void {}
}

class Child extends Base
{
    protected static function mine(): void {}
}
"#,
        expect![""],
    );
}

/// A constructor is exempt from all of it, which is what makes dependency injection in a framework
/// possible: every subclass declares whatever it needs injected.
#[test]
fn a_constructor_is_exempt() {
    check(
        r#"
//- /app.php
<?php
class Base
{
    public function __construct(int $a, int $b) {}
}

class Child extends Base
{
    protected function __construct() {}
}
"#,
        expect![""],
    );
}

#[test]
fn each_way_an_override_can_be_refused() {
    check(
        r#"
//- /app.php
<?php
class Base
{
    public function open(int $a, int $b): void {}

    public function fine(int $a): void {}

    public static function stat(): void {}

    final public function sealed(): void {}
}

class Bad extends Base
{
    protected function open(int $a, int $b): void {}

    public function fine(): void {}

    public function stat(): void {}

    public function sealed(): void {}
}

class AddsRequired extends Base
{
    public function fine(int $a, int $extra): void {}
}
"#,
        expect![[r#"
            15:24-15:28 error [bad-override] open() cannot override Base::open(): public cannot narrow to protected
            17:21-17:25 error [bad-override] fine() cannot override Base::fine(): it takes 0 parameters where 1 is declared
            19:21-19:25 error [bad-override] stat() cannot override Base::stat(): the declaration above it is static and this is not
            21:21-21:27 error [bad-override] sealed() cannot override Base::sealed(): the declaration above it is final
            26:21-26:25 error [bad-override] fine() cannot override Base::fine(): $extra is required here and the declaration above it has no such parameter
        "#]],
    );
}

/// An interface is a declaration to fit too, and the commonest one in framework code.
#[test]
fn an_interface_method_is_a_declaration_to_fit() {
    check(
        r#"
//- /app.php
<?php
interface Handler
{
    public function handle(string $event, int $at): void;
}

class Narrow implements Handler
{
    public function handle(string $event): void {}
}
"#,
        expect![[r#"
            9:21-9:27 error [bad-override] handle() cannot override Handler::handle(): it takes 1 parameter where 2 are declared
        "#]],
    );
}

/// A `@mixin` forwards calls and declares nothing above the class, so a subclass writing one of its
/// names overrides nothing -- Laravel's model stubs declare `where($key, $value)` beside a builder's
/// four-parameter one. The real override beside it is the control that the check ran.
#[test]
fn a_method_a_mixin_forwards_is_not_overridden() {
    check(
        r#"
//- /app.php
<?php
class Builder
{
    public function where($column, $operator = null, $value = null, $boolean = 'and') {}
}

/** @mixin Builder */
class Model
{
    public function save(array $options = []): bool { return true; }
}

class Stub extends Model
{
    public function where($key, $value) {}

    public function save(): bool { return true; }
}
"#,
        expect![[r#"
            17:21-17:25 error [bad-override] save() cannot override Model::save(): it takes 0 parameters where 1 is declared
        "#]],
    );
}
