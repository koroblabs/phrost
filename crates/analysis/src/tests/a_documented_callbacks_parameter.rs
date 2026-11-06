//! `singleton(Foo::class, function ($app) { ... })` -- a closure's parameter, typed by what the
//! callee said it would be called with.
//!
//! PHP has no syntax for it, so a `@param callable(Application): void` is the only thing that can
//! say what the closure receives. Until it was read, every service provider in every container-based
//! framework had an untyped `$app`: 130 `function ($app)` sites in Laravel's own `src/` alone, plus
//! one in every application's providers.
//!
//! Two sources answer this question and they are ranked, so both are here. The declared signature is
//! a statement and wins; the element type of a container passed beside the callback is a guess that
//! is right for `array_map` and says nothing for a call whose other argument is a class name. The
//! second test is the one that would have caught the ranking being built the other way round --
//! preferring a source that is always *available* would have taken the container reading out of
//! every `array_map` in the project without failing anything.
//!
//! Measured with `undefined-method`, because a receiver that resolves to nothing is silent: hover
//! through the CLI never lands on the parameter, and an argument whose type is unknown is accepted
//! everywhere. A method that does not exist on the type the tag names is the only thing that
//! separates "the parameter is typed" from "the parameter is untyped".

use expect_test::expect;

use super::diagnostics::check;

const APP: &str = r#"
//- /app.php
<?php
class Application
{
    public function make(string $id): object
    {
        return new self();
    }
}

class Container
{
    /**
     * @param callable(Application): void $factory
     */
    public function singleton(string $abstract, callable $factory): void
    {
    }
}
"#;

/// The tag types the closure's parameter, so a call on it is checked against `Application`.
#[test]
fn a_declared_callable_signature_types_the_closures_parameter() {
    check(
        &[
            APP,
            r#"
//- /provider.php
<?php
function register(Container $container): void
{
    $container->singleton('config', function ($app) {
        $app->make('other');
        $app->nope();
    });
}
"#,
        ]
        .concat(),
        expect![[r#"
            6:15-6:19 error [undefined-method] Undefined method Application::nope()
        "#]],
    );
}

/// With the callee resolved and documenting no signature, the container beside it still answers.
///
/// The callee resolves here on purpose. Written against the real `array_map` the fixture has no
/// stubs, so the declaration is not found at all -- which exercises "no declaration" rather than
/// "a declaration that says nothing about the callback", and those are different halves of the
/// ranking.
#[test]
fn a_container_beside_the_callback_still_types_the_first_parameter() {
    check(
        &[
            APP,
            r#"
//- /mapped.php
<?php
function eachOf(callable $fn, array $items): void
{
}

/**
 * @param Application[] $apps
 */
function mapped(array $apps): void
{
    eachOf(function ($app) {
        $app->make('other');
        $app->nope();
    }, $apps);
}
"#,
        ]
        .concat(),
        expect![[r#"
            13:15-13:19 error [undefined-method] Undefined method Application::nope()
        "#]],
    );
}

/// Where both sources could answer, the declared one does.
///
/// The test that would have caught the ranking built the other way round: the sibling holds
/// `Application[]` and the tag names `Rule`, so whichever source won is the one the diagnostic
/// names. Nothing else distinguishes them -- both resolve, both answer, and a project where they
/// agree is every project until one does not.
#[test]
fn a_declared_signature_beats_the_container_beside_it() {
    check(
        &[
            APP,
            r#"
//- /ranked.php
<?php
class Rule
{
    public function name(): string
    {
        return '';
    }
}

/**
 * @param callable(Rule): void $fn
 * @param Application[] $items
 */
function eachRule(callable $fn, array $items): void
{
}

function ranked(array $apps): void
{
    eachRule(function ($one) {
        $one->name();
        $one->nope();
    }, $apps);
}
"#,
        ]
        .concat(),
        expect![[r#"
            22:15-22:19 error [undefined-method] Undefined method Rule::nope()
        "#]],
    );
}

/// A position the tag named beyond the first, which the container reading cannot answer at all.
#[test]
fn a_declared_signature_types_a_parameter_past_the_first() {
    check(
        &[
            APP,
            r#"
//- /second.php
<?php
class Walker
{
    /**
     * @param callable(string, Application): void $each
     */
    public function each(callable $each): void
    {
    }
}

function walk(Walker $walker): void
{
    $walker->each(function ($key, $app) {
        $app->make($key);
        $app->nope();
    });
}
"#,
        ]
        .concat(),
        expect![[r#"
            16:15-16:19 error [undefined-method] Undefined method Application::nope()
        "#]],
    );
}

/// The closure's own declaration bounds whatever is inferred for it.
///
/// PSL's `Type\converted(TypeInterface<I> $from, TypeInterface<O> $into, Closure(I): O)`: the
/// template is held by `$from`, so the call site answers -- and with `$from` typed by nothing it
/// answered with the element of `$into`, the other container, giving `static fn (array $data)` a
/// `Person` and reporting every subscript on it as an object used as an array. PHP checks the
/// `array` before the body runs, so an inference contradicting it is impossible.
#[test]
fn an_inference_the_closures_own_declaration_contradicts_gives_way() {
    check(
        r#"
//- /src/types.php
<?php
namespace App;

/** @template T */
interface TypeInterface {}

/**
 * @template T
 * @param class-string<T> $class
 * @return TypeInterface<T>
 */
function instance_of(string $class): TypeInterface {}

function shape(array $elements) {}

/**
 * @template I
 * @template O
 * @param TypeInterface<I> $from
 * @param TypeInterface<O> $into
 * @param (\Closure(I): O) $converter
 * @return TypeInterface<O>
 */
function converted(TypeInterface $from, TypeInterface $into, \Closure $converter): TypeInterface {}

//- /src/Person.php
<?php
namespace App;

final class Person
{
    public function __construct(public string $first) {}

    public static function type(): TypeInterface
    {
        return converted(
            shape([]),
            instance_of(Person::class),
            static fn (array $data): Person => new Person($data['first']),
        );
    }
}
"#,
        expect![[""]],
    );
}
