//! Where the arguments of `new class(...)` are written.
//!
//! ```php
//! $processor = new class(
//!   $this->createMock(EntityTypeManagerInterface::class),
//! ) extends OrderResourceDataProcessor {
//! ```
//!
//! PHP evaluates them in the scope the `new` is in, so `$this` there is the *enclosing* object --
//! a test case, in the shape every PHPUnit suite writes it. Read from the declaration's extent they
//! belonged to the anonymous class instead, and `$this->createMock(...)` reported
//! `Undefined method class@anonymous#3129::createMock()`: five findings in one real test file.
//!
//! The braces are what decide which class an offset is in, and the pair below is the whole rule --
//! the argument sees the enclosing class, the body sees the anonymous one.

use expect_test::expect;

use super::Query;
use super::diagnostics::check as check_diagnostics;

const FIXTURE: &str = r#"
//- /app.php
<?php

class Base
{
    public function run(): void {}
}

class Mock {}

class TestCase
{
    public function createMock(string $class): Mock
    {
        return new Mock();
    }

    public function helper(): string
    {
        return 'x';
    }

    public function test(): void
    {
        $processor = new class($this->createMock(Base::class), $this->hel@here@per()) extends Base {
            public function run(): void
            {
                $this->own();
            }

            private function own(): void {}
        };
        $processor->run();
    }
}
"#;

#[test]
fn an_argument_sees_the_class_the_new_is_written_in() {
    check_diagnostics(FIXTURE, expect![[r#""#]]);
}

#[test]
fn and_the_body_sees_the_anonymous_one() {
    // The other half: `$this->own()` inside the body is the anonymous class's own private method,
    // which is only reachable if the body still belongs to it.
    super::check(
        FIXTURE,
        &[Query::hover("the enclosing call in an argument", "here")],
        expect![[r#"
            the enclosing call in an argument
            - code: <?php
            class TestCase
            {
                public function helper(): string
            }
        "#]],
    );
}
