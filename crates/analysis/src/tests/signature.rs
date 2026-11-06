//! What to show while a call is being written.
//!
//! Two things have to be right, and only one of them is resolution. The other is which parameter
//! the cursor is filling — and being wrong about that is worse than saying nothing, because a
//! highlighted parameter is read as a statement about the argument being typed.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_method_call_shows_its_parameters_and_marks_the_one_being_filled() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id, bool $strict = false): void {}

    public function run(): void
    {
        $this->find(1, @here@);
    }
}
"#,
        &[Query::signature("second argument", "here")],
        expect![[r#"
            second argument
            - find(int $id, bool $strict = false)
                int $id
              > bool $strict = false
        "#]],
    );
}

#[test]
fn the_first_parameter_is_marked_immediately_after_the_paren() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id, bool $strict = false): void {}

    public function run(): void
    {
        $this->find(@here@);
    }
}
"#,
        &[Query::signature("nothing typed yet", "here")],
        expect![[r#"
            nothing typed yet
            - find(int $id, bool $strict = false)
              > int $id
                bool $strict = false
        "#]],
    );
}

#[test]
fn a_constructor_call_shows_the_constructors_parameters() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function __construct(private string $dsn, private int $timeout = 5) {}
}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function make(): Repo
    {
        return new Repo('sqlite', @here@);
    }
}
"#,
        &[Query::signature("constructor", "here")],
        expect![[r#"
            constructor
            - __construct(private string $dsn, private int $timeout = 5)
                private string $dsn
              > private int $timeout = 5
        "#]],
    );
}

#[test]
fn a_free_function_shows_its_declared_parameters() {
    check(
        r#"
//- /src/helpers.php
<?php
namespace App;

function attach(string $label, int $weight, bool $sticky = false): void {}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function run(): void
    {
        attach('a', @here@);
    }
}
"#,
        &[Query::signature("free function", "here")],
        expect![[r#"
            free function
            - attach(string $label, int $weight, bool $sticky = false)
                string $label
              > int $weight
                bool $sticky = false
        "#]],
    );
}

#[test]
fn the_inner_call_is_the_one_being_written() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function outer(string $a, string $b): void {}

    public function inner(int $one, int $two): void {}

    public function run(): void
    {
        $this->outer('x', $this->inner(1, @here@));
    }
}
"#,
        &[Query::signature("innermost", "here")],
        expect![[r#"
            innermost
            - inner(int $one, int $two)
                int $one
              > int $two
        "#]],
    );
}

#[test]
fn past_a_named_argument_no_parameter_is_marked() {
    // Position has stopped meaning anything: `strict:` may fill any parameter, so the argument
    // after it is not the one at that index. Marking it would point at the wrong parameter.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id, bool $strict = false, ?string $tag = null): void {}

    public function run(): void
    {
        $this->find(strict: true, @here@);
    }
}
"#,
        &[Query::signature("after a named argument", "here")],
        expect![[r#"
            after a named argument
            - find(int $id, bool $strict = false, ?string $tag = null)
                int $id
                bool $strict = false
                ?string $tag = null
              (no parameter is being filled)
        "#]],
    );
}

#[test]
fn a_variadic_keeps_taking_arguments_past_its_own_position() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function log(string $message, string ...$context): void {}

    public function run(): void
    {
        $this->log('a', 'b', 'c', @here@);
    }
}
"#,
        &[Query::signature("fourth argument", "here")],
        expect![[r#"
            fourth argument
            - log(string $message, string ...$context)
                string $message
              > string ...$context
        "#]],
    );
}

#[test]
fn a_call_on_a_receiver_with_no_known_type_shows_nothing() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run($unknown): void
    {
        $unknown->whatever(@here@);
    }
}
"#,
        &[Query::signature("unresolved receiver", "here")],
        expect![[r#"
            unresolved receiver
            - (no signature)
        "#]],
    );
}

#[test]
fn a_call_written_inside_a_closure_is_still_a_call() {
    // The walk that finds calls used to stop at a closure body, so signature help and named-argument
    // completion were both dead inside every callback -- and `array_map(fn ($row) => ..., $rows)` is
    // where half of them are written.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id, bool $strict): void {}
}

class Service
{
    public function run(Repo $repo, array $rows): void
    {
        $mapped = array_map(fn ($row) => $repo->find($0), $rows);
    }
}
"#,
        &[Query::signature("inside an arrow function", "0")],
        expect![[r#"
            inside an arrow function
            - find(int $id, bool $strict)
              > int $id
                bool $strict
        "#]],
    );
}

#[test]
fn a_cursor_on_a_nested_callee_name_is_still_filling_the_outer_argument() {
    // The nested call contains the cursor, but the cursor is on its *name* — so nothing of it is
    // being written yet, and what is being filled is the outer call's second argument. Answering
    // with the inner one highlighted a parameter belonging to another function.
    check(
        r#"
//- /app.php
<?php
function hello(string $foo, int $bar): void {}
function goodbye(string $good, int $by): void {}

function run(): void
{
    hello(
        "hello",
        goo@here@dbye("good", "by")
    );
}
"#,
        &[Query::signature("on the nested callee name", "here")],
        expect![[r#"
            on the nested callee name
            - hello(string $foo, int $bar)
                string $foo
              > int $bar
        "#]],
    );
}

#[test]
fn a_cursor_inside_the_nested_call_belongs_to_the_nested_call() {
    check(
        r#"
//- /app.php
<?php
function hello(string $foo, int $bar): void {}
function goodbye(string $good, int $by): void {}

function run(): void
{
    hello(
        "hello",
        goodbye("good", @here@)
    );
}
"#,
        &[Query::signature("inside the nested arguments", "here")],
        expect![[r#"
            inside the nested arguments
            - goodbye(string $good, int $by)
                string $good
              > int $by
        "#]],
    );
}

#[test]
fn an_attribute_shows_its_constructor_parameters() {
    check(
        r#"
//- /app.php
<?php
#[Attribute]
class Route
{
    public function __construct(string $path, string $name = '') {}
}

#[Route(@here@)]
class Home {}
"#,
        &[Query::signature("writing an attribute", "here")],
        expect![[r#"
            writing an attribute
            - __construct(string $path, string $name = '')
              > string $path
                string $name = ''
        "#]],
    );
}

#[test]
fn an_attributes_second_parameter_is_marked_after_the_comma() {
    check(
        r#"
//- /app.php
<?php
#[Attribute]
class Route
{
    public function __construct(string $path, string $name = '') {}
}

#[Route('/home', @here@)]
class Home {}
"#,
        &[Query::signature("second attribute argument", "here")],
        expect![[r#"
            second attribute argument
            - __construct(string $path, string $name = '')
                string $path
              > string $name = ''
        "#]],
    );
}
