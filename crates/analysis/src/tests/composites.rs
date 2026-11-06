//! A receiver written as a union or an intersection of classes.
//!
//! Two shapes that look alike and reduce differently. `A|B` says the value is one of them, so what
//! can be reached through it is what they have in common -- their nearest common ancestor. `A&B`
//! says the value is all of them, so it has both APIs at once and does not reduce to one class at
//! all; that one is a miss, and this file says so rather than leaving it undescribed.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_union_reaches_what_the_operands_share() {
    check(
        r#"
//- /app.php
<?php
class Failure extends \RuntimeException {}

class Service
{
    public function run(): void
    {
        try {
            $this->work();
        }
        catch (Failure | \LogicException $error) {
            $error->getMess$0age();
        }
    }

    private function work(): void {}
}
//- /stubs.php
<?php
class Exception
{
    public function getMessage(): string {}
}
class RuntimeException extends Exception {}
class LogicException extends Exception {}
"#,
        &[Query::goto("caught by two classes", "0")],
        expect![[r#"
            caught by two classes
            - method Exception::getMessage @ stubs.php 4:21-4:31 (full 4:5-4:44)
        "#]],
    );
}

#[test]
fn a_union_of_unrelated_classes_reaches_only_what_both_declare() {
    // Nothing above both, so the only members the value is guaranteed to have are the ones every
    // operand declares. `name` is one of those, so it resolves -- and goto answers with the first
    // operand, which is one of the places the reader may land rather than all of them.
    check(
        r#"
//- /app.php
<?php
class Alpha
{
    public function name(): string {}
}
class Beta
{
    public function name(): string {}
}

class Service
{
    /**
     * @param Alpha|Beta $either
     */
    public function run($either): void
    {
        $either->na$0me();
    }
}
"#,
        &[Query::goto("two unrelated classes", "0")],
        expect![[r#"
            two unrelated classes
            - method Alpha::name @ app.php 4:21-4:25 (full 4:5-4:38)
        "#]],
    );
}

#[test]
fn an_intersection_reaches_both_of_its_operands() {
    // `Application&MockObject` says the value is all of them, so it has both APIs at once -- and
    // picking one operand, which is what reducing it would mean, loses the other's entirely. Every
    // mock in a modern PHPUnit test is declared this way.
    check(
        r#"
//- /app.php
<?php
interface MockObject
{
    public function expects(): void;
}
class Application
{
    public function id(): int {}
}

class Test
{
    public function run(Application&MockObject $mock): void
    {
        $mock->exp@mocked@ects();
        $mock->i@real@d();
        $mock->@offer@
    }
}
"#,
        &[
            Query::goto("from the mock's own interface", "mocked"),
            Query::goto("from the class being mocked", "real"),
            Query::completion("both at once", "offer"),
        ],
        expect![[r#"
            from the mock's own interface
            - method MockObject::expects @ app.php 4:21-4:28 (full 4:5-4:37)
            from the class being mocked
            - method Application::id @ app.php 8:21-8:23 (full 8:5-8:33)
            both at once
            - id [method]  : int  writes `();`
            - expects [method]  : void  writes `();`
            - accepting the first leaves `$mock->id`
        "#]],
    );
}
