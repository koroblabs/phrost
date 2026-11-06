//! What a call promises about what it was passed.
//!
//! `@phpstan-assert` is how a codebase writes down the thing PHP cannot express: a helper whose
//! whole purpose is to prove something about its argument. The claim is written on the callee, in
//! whatever file that is, so the call site names the call and the reader looks the tag up.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn an_assertion_replaces_what_the_declaration_said() {
    check(
        r#"
//- /src/Assert.php
<?php
namespace App;

class Foobar
{
    public function fooMethod(): void {}
}

final class Assert
{
    /**
     * @phpstan-assert Foobar $value
     */
    public static function isFoobar(mixed $value): void {}
}

class Runner
{
    public function run(object $thing): void
    {
        Assert::isFoobar($thing);
        $thing->@here@
    }
}
"#,
        &[Query::completion("after the contract", "here")],
        expect![[r#"
            after the contract
            - fooMethod [method]  : void  writes `();`
            - accepting the first leaves `$thing->fooMethod`
        "#]],
    );
}

#[test]
fn a_negated_assertion_rules_an_operand_out() {
    check(
        r#"
//- /src/Not.php
<?php
namespace App;

class Foobar
{
    public function fooMethod(): void {}
}

class Barfoo
{
    public function barMethod(): void {}
}

final class Refute
{
    /**
     * @phpstan-assert !Foobar $value
     */
    public static function isNotFoobar(mixed $value): void {}
}

class Runner
{
    public function run(Foobar|Barfoo $thing): void
    {
        Refute::isNotFoobar($thing);
        $thing->@here@
    }
}
"#,
        &[Query::completion("the one left", "here")],
        expect![[r#"
            the one left
            - barMethod [method]  : void  writes `();`
            - accepting the first leaves `$thing->barMethod`
        "#]],
    );
}

#[test]
fn an_assertion_naming_a_class_nothing_declares_says_nothing() {
    // `@phpstan-assert =ExpectedType $actual` names a `@template` parameter, which is a class name
    // only once the argument that bound it has been substituted. Replacing the value with it would
    // take away the type the declaration did give.
    check(
        r#"
//- /src/Generic.php
<?php
namespace App;

class Node
{
    public function image(): string { return ''; }
}

class Asserts
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert =ExpectedType $actual
     */
    public static function instanceOf(string $expected, object $actual): void {}
}

class Runner
{
    public function run(string $expected, ?Node $node): void
    {
        Asserts::instanceOf($expected, $node);
        $node->@here@
    }
}
"#,
        &[Query::completion("a template is not a class", "here")],
        expect![[r#"
            a template is not a class
            - image [method]  : string  writes `();`
            - accepting the first leaves `$node->image`
        "#]],
    );
}

#[test]
fn a_call_that_asserts_nothing_leaves_the_value_alone() {
    check(
        r#"
//- /src/Quiet.php
<?php
namespace App;

class Foobar
{
    public function fooMethod(): void {}
}

final class Quiet
{
    public static function look(mixed $value): void {}
}

class Runner
{
    public function run(object $thing): void
    {
        Quiet::look($thing);
        $thing->@here@
    }
}
"#,
        &[Query::completion("no tag on the callee", "here")],
        expect![[r#"
            no tag on the callee
            - (nothing offered)
        "#]],
    );
}

#[test]
fn an_assertion_can_name_the_class_at_the_call_instead() {
    // PHPUnit's `assertInstanceOf`, and every project's copy of it. What `$actual` is proved to be
    // is written at the call as `Foo::class`; the declaration only says which parameter carries it.
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

class BaseAssert
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert ExpectedType $actual
     */
    public static function assertInstanceOf(string $expected, object $actual): void {}
}
//- /src/Case.php
<?php
namespace App;

class Node
{
    public function image(): string { return ''; }
}

class MyCase extends BaseAssert
{
    public function testIt(object $thing): void
    {
        self::assertInstanceOf(Node::class, $thing);
        $thing->@here@
    }
}
"#,
        &[Query::completion("the class came from the call", "here")],
        expect![[r#"
            the class came from the call
            - image [method]  : string  writes `();`
            - accepting the first leaves `$thing->image`
        "#]],
    );
}

#[test]
fn a_call_that_names_no_class_there_asserts_nothing() {
    // `assertInstanceOf($expectedClass, $thing)` passes a variable, so nothing at the call says
    // which class -- and the tag on its own names a `@template`, which is not one.
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

class BaseAssert
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert ExpectedType $actual
     */
    public static function assertInstanceOf(string $expected, object $actual): void {}
}
//- /src/Case.php
<?php
namespace App;

class Node
{
    public function image(): string { return ''; }
}

class MyCase extends BaseAssert
{
    public function testIt(string $expected, object $thing): void
    {
        self::assertInstanceOf($expected, $thing);
        $thing->@here@
    }
}
"#,
        &[Query::completion("a variable class name", "here")],
        expect![[r#"
            a variable class name
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_factory_returns_the_class_the_call_named() {
    // A DI container's `get()`, a test's `createMock()`, a repository factory. The declaration says
    // `@return T` and only the call says what `T` is.
    check(
        r#"
//- /src/Factory.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Factory
{
    /**
     * @template T
     * @param class-string<T> $class
     * @return T
     */
    public function make(string $class): object {}
}

class Runner
{
    public function run(Factory $factory): void
    {
        $made = $factory->make(Repo::class);
        $made->@here@
    }
}
"#,
        &[Query::completion("what the call named", "here")],
        expect![[r#"
            what the call named
            - find [method]  : void  writes `();`
            - accepting the first leaves `$made->find`
        "#]],
    );
}

#[test]
fn a_factory_called_with_a_variable_returns_nothing() {
    check(
        r#"
//- /src/Factory.php
<?php
namespace App;

class Factory
{
    /**
     * @template T
     * @param class-string<T> $class
     * @return T
     */
    public function make(string $class): object {}
}

class Runner
{
    public function run(Factory $factory, string $wanted): void
    {
        $made = $factory->make($wanted);
        $made->@here@
    }
}
"#,
        &[Query::completion("no class at the call", "here")],
        expect![[r#"
            no class at the call
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_factory_can_name_its_class_at_any_argument() {
    // Which argument decides is the declaration's business, and a `class-string<T>` parameter is
    // written wherever the signature wanted it. A literal at some other position is a different
    // argument and reading it would name the wrong class.
    check(
        r#"
//- /src/Registry.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Registry
{
    /**
     * @template T
     * @param class-string<T> $class
     * @return T
     */
    public function make(string $label, string $class): object {}
}

class Runner
{
    public function run(Registry $registry): void
    {
        $made = $registry->make('repositories', Repo::class);
        $made->@here@
    }
}
"#,
        &[Query::completion("named at the second argument", "here")],
        expect![[r#"
            named at the second argument
            - find [method]  : void  writes `();`
            - accepting the first leaves `$made->find`
        "#]],
    );
}

/// An override that repeats the signature and not the docblock above it is not withdrawing what the
/// interface promised -- which is what every implementation of a documented interface looks like.
#[test]
fn a_contract_is_inherited_by_an_override_that_says_nothing() {
    check(
        r#"
//- /src/Container.php
<?php
namespace App;

class Repo
{
    public function all(): array {}
}

interface ContainerInterface
{
    /**
     * @template T of object
     * @param class-string<T> $id
     * @return T
     */
    public function get(string $id): object;
}

class Container implements ContainerInterface
{
    public function get(string $id): object {}
}

class Runner
{
    public function run(Container $container): void
    {
        $repo = $container->get(Repo::class);
        $repo->@inherited@
    }
}
"#,
        &[Query::completion("promised by the interface", "inherited")],
        expect![[r#"
            promised by the interface
            - all [method]  : array  writes `();`
            - accepting the first leaves `$repo->all`
        "#]],
    );
}

/// A chain has no expression behind it -- the statement being typed does not parse -- so the
/// arguments come out of the text. Which is the only place they can come from, and the only reason a
/// factory in the middle of a chain resolves at all.
#[test]
fn a_chained_factory_reads_the_class_its_argument_names() {
    check(
        r#"
//- /src/Chain.php
<?php
namespace App;

class Repo
{
    public function all(): array {}
}

interface ContainerInterface
{
    /**
     * @template T of object
     * @param class-string<T> $id
     * @return T
     */
    public function get(string $id): object;
}

class Container implements ContainerInterface
{
    public function get(string $id): object {}
}

class Runner
{
    public function run(Container $container): void
    {
        $container->get(Repo::class)->@chained@
    }
}
"#,
        &[Query::completion("named in the chain", "chained")],
        expect![[r#"
            named in the chain
            - all [method]  : array  writes `();`
            - accepting the first leaves `$container->get(Repo::class)->all`
        "#]],
    );
}

/// The plainest generic there is: whatever was passed is what comes back. Only the call says what
/// that was, so the declaration can name nothing but the argument to read it from.
#[test]
fn a_return_bound_to_a_parameters_own_type_reads_the_argument() {
    check(
        r#"
//- /src/Same.php
<?php
namespace App;

class Item
{
    public function label(): string {}
}

class Helper
{
    /**
     * @template T
     * @param T $value
     * @return T
     */
    public function through($value) {}
}

class Runner
{
    public function run(Helper $helper, Item $item): void
    {
        $back = $helper->through($item);
        $back->@same@
    }
}
"#,
        &[Query::completion("whatever went in", "same")],
        expect![[r#"
            whatever went in
            - label [method]  : string  writes `();`
            - accepting the first leaves `$back->label`
        "#]],
    );
}

/// The same claim about a container's element: `@param array<T> $rows` with `@return T`.
#[test]
fn a_return_bound_to_a_parameters_element_reads_what_it_holds() {
    check(
        r#"
//- /src/First.php
<?php
namespace App;

class Config
{
    public function address(): string {}
}

/**
 * @template T
 * @param array<T> $rows
 * @return T
 */
function first_of(array $rows) {}

class Runner
{
    /** @return array<string, Config> */
    private function configs(): array {}

    public function run(): void
    {
        $one = first_of($this->configs());
        $one->@element@
    }
}
"#,
        &[Query::completion("the element of what went in", "element")],
        expect![[r#"
            the element of what went in
            - address [method]  : string  writes `();`
            - accepting the first leaves `$one->address`
        "#]],
    );
}
