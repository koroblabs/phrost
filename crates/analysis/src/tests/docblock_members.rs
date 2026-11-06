//! Members a class declares in a comment.
//!
//! How a library describes a surface it produces at runtime. PHPUnit's mocks are the case that
//! forced it: `$mock->method('x')` is the most-written line in any test suite, and `method()` exists
//! only as an `@method` tag on an interface.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_method_tag_is_navigable_and_callable() {
    check(
        r#"
//- /src/app.php
<?php

/**
 * @method Stubber method(string $constraint)
 */
interface Stub
{
}

class Service
{
    public function run(Stub $stub): void
    {
        $stub->@call@method('x');
    }
}
"#,
        &[
            Query::goto("goto", "call"),
            Query::hover("hover", "call"),
            Query::completion("completion", "call"),
        ],
        expect![[r#"
            goto
            - method Stub::method @ src/app.php 4:20-4:26 (full 4:12-4:46)
            hover
            - code: <?php
            interface Stub
            {
                public function method(string $constraint): Stubber
            }
            - note: declared by a docblock tag
            completion
            - method [method]  : Stubber  writes `(|)`
            - accepting the first leaves `$stub->methodmethod('x');`
        "#]],
    );
}

#[test]
fn a_written_declaration_wins_over_a_tag_claiming_the_same_name() {
    check(
        r#"
//- /src/app.php
<?php

/**
 * @method string run()
 */
class Service
{
    public function run(): int { return 1; }

    public function go(): void
    {
        $this->@call@run();
    }
}
"#,
        &[Query::hover("hover", "call")],
        expect![[r#"
            hover
            - code: <?php
            class Service
            {
                public function run(): int
            }
        "#]],
    );
}

#[test]
fn a_property_tag_carries_its_type_into_a_chain() {
    check(
        r#"
//- /src/app.php
<?php

class Repo
{
    public function find(): void {}
}

/**
 * @property Repo $repo
 */
class Container
{
}

class Service
{
    public function run(Container $container): void
    {
        $container->repo->@here@
    }
}
"#,
        &[Query::completion("through a magic property", "here")],
        expect![[r#"
            through a magic property
            - find [method]  : void  writes `();`
            - accepting the first leaves `$container->repo->find`
        "#]],
    );
}
