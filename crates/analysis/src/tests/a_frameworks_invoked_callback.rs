//! `$this->app->singleton(Foo::class, function ($app) { ... })` -- a closure's parameter typed by
//! what the framework says it hands the callback.
//!
//! The other source for the fact `@param callable(A): B` already supplies, read through the same
//! view. It exists because a framework's most-used callback-takers document nothing:
//! `Container::singleton` says `\Closure|string|null` and calls the closure with the container,
//! `Schema::create` says `\Closure` and calls it with a `Blueprint`. So a service provider's
//! `$app` and a migration's `$table` have no type at all, and a Laravel project is mostly those
//! two shapes -- 226 and 250 sites in the framework's own source.
//!
//! Stated here the way the server states it, through `World::contracts_mut`, so the test exercises
//! the index table and the view rather than the provider's list. What the list itself says is
//! checked where it is written.

use expect_test::expect;

use super::diagnostics::check_framework;

const APP: &str = r#"
//- /app.php
<?php
class Application
{
    public function environment(): string
    {
        return 'local';
    }
}

class Container
{
    public function singleton(string $abstract, $concrete = NULL): void
    {
    }

    public function extend(string $abstract, callable $closure): void
    {
    }
}
"#;

/// The contract types the closure's parameter, so a call on it is checked.
#[test]
fn a_contract_types_the_callbacks_parameter() {
    check_framework(
        &[
            APP,
            r#"
//- /provider.php
<?php
function register(Container $container): void
{
    $container->singleton('config', function ($app) {
        $app->environment();
        $app->nope();
    });
}
"#,
        ]
        .concat(),
        |world| {
            world.contracts_mut().add_invoked_callback(
                "Container",
                "singleton",
                1,
                &[Some("Application")],
            );
        },
        expect![[r#"
            6:15-6:19 error [undefined-method] Undefined method Application::nope()
        "#]],
    );
}

/// A position the contract leaves unnamed keeps its place, so the one after it is still right.
///
/// `extend` is the shape that forces the holes: it calls `$closure($service, $container)`, and the
/// service's class is whatever was bound and unknowable from a contract. Dropping the hole would
/// put the container on `$service` -- typing the wrong parameter, with nothing failing.
#[test]
fn an_unnamed_position_holds_its_place() {
    check_framework(
        &[
            APP,
            r#"
//- /decorator.php
<?php
function decorate(Container $container): void
{
    $container->extend('config', function ($service, $app) {
        $app->environment();
        $app->nope();
    });
}
"#,
        ]
        .concat(),
        |world| {
            world.contracts_mut().add_invoked_callback(
                "Container",
                "extend",
                1,
                &[None, Some("Application")],
            );
        },
        expect![[r#"
            6:15-6:19 error [undefined-method] Undefined method Application::nope()
        "#]],
    );
}

/// A closure that declares its own parameter type keeps it, and the contract never runs.
///
/// The discipline a contributed rule has to follow -- it may not contradict a declaration -- and
/// here it is satisfied upstream rather than by a check: the root that reaches the contract exists
/// only for a parameter whose own declaration is absent or a bare container.
#[test]
fn a_declared_parameter_is_not_overridden() {
    check_framework(
        &[
            APP,
            r#"
//- /hinted.php
<?php
function hinted(Container $container): void
{
    $container->singleton('config', function (Container $app) {
        $app->singleton('other');
        $app->environment();
    });
}
"#,
        ]
        .concat(),
        |world| {
            world.contracts_mut().add_invoked_callback(
                "Container",
                "singleton",
                1,
                &[Some("Application")],
            );
        },
        expect![[r#"
            6:15-6:26 error [undefined-method] Undefined method Container::environment()
        "#]],
    );
}

/// A contract naming a class the project does not declare says nothing.
///
/// What makes an entry for `Illuminate\Foundation\Application` safe in a project that vendors
/// `illuminate/container` without the foundation: there the callback really does receive a bare
/// container, so answering nothing is right rather than a miss. Reported as an untyped receiver --
/// which is silence -- so the control is the *absence* of the finding the first test has.
#[test]
fn a_contract_naming_an_undeclared_class_says_nothing() {
    check_framework(
        &[
            APP,
            r#"
//- /absent.php
<?php
function absent(Container $container): void
{
    $container->singleton('config', function ($app) {
        $app->nope();
    });
}
"#,
        ]
        .concat(),
        |world| {
            world.contracts_mut().add_invoked_callback(
                "Container",
                "singleton",
                1,
                &[Some("Vendor\\Absent\\Application")],
            );
        },
        expect![[r#""#]],
    );
}
