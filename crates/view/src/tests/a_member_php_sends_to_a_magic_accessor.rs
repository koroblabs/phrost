//! A member this scope cannot touch, on a class that answers anything.
//!
//! PHP does not refuse the call -- it sends it to `__call`, which is a different method returning a
//! different thing. So reading the declaration there is reading a method that never runs, and the
//! value that comes back is whatever the accessor forwards to.
//!
//! Laravel's `Router` is the shape: `prefix()` is **protected** and returns a `string`, and its
//! `__call` hands the name to a `RouteRegistrar` which returns itself. `$router->prefix('x')->get(...)`
//! therefore answered `string` and the next call in the chain was reported as one made on it.
//!
//! The visibility check already made this exemption -- it is the difference between reporting an
//! inaccessible member and reporting every framework class with a magic accessor -- and the type
//! resolution did not. One rule, two places, and only one of them knew.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_protected_method_reached_from_outside_does_not_supply_its_type() {
    check_view(
        r#"
//- /src/Registry.php
<?php
namespace App;

class Registrar
{
    public function get(string $path): Route
    {
    }
}

class Route
{
    public function uri(): string
    {
    }
}

class Router
{
    protected function prefix(string $path): string
    {
    }

    public function __call(string $name, array $arguments): Registrar
    {
    }

    public function inside(): void
    {
        $o@own@wn = $this->prefix('x');
    }
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(Router $router): void
    {
        $f@forwarded@orwarded = $router->prefix('x');
    }
}
"#,
        &[
            Query::variable_type("the declaration, from inside the class", "own"),
            Query::variable_type("`__call`, from outside it", "forwarded"),
        ],
        expect![[r#"
            the declaration, from inside the class
            - string
            `__call`, from outside it
            - (no type)
        "#]],
    );
}

/// And the half that keeps the exemption honest: a class with no magic accessor is reached by its
/// declaration or not at all, and the declaration is still what says what it hands back.
#[test]
fn a_protected_method_on_a_class_with_no_accessor_still_names_its_type() {
    check_view(
        r#"
//- /src/Plain.php
<?php
namespace App;

class Plain
{
    protected function label(): string
    {
    }
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(Plain $plain): void
    {
        $l@label@abel = $plain->label();
    }
}
"#,
        &[Query::variable_type("no accessor to forward to", "label")],
        expect![[r#"
            no accessor to forward to
            - string
        "#]],
    );
}
