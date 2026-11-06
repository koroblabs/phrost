//! A catalog key that holds a value rather than naming a class.
//!
//! A container has two of them. A service id names the class the container builds, and
//! `ContainerInterface::get()` reads as that class. A **parameter** holds a value, and
//! `getParameter()` can only declare `array|bool|string|int|float|\UnitEnum|null` -- every kind a
//! configuration value can be, because a signature cannot say which parameter it is being asked
//! about. What can say is the configuration: `cache_default_bin_backends: []` is an array and
//! `security.enable_super_user: true` is a bool, and `services.rs` reads the shape out of the same
//! YAML the service ids come from.
//!
//! So the shape travels as the key's value, beside the class and the place a key can also name, and
//! the call reads it. Without it, every `getParameter()` handed to a typed parameter reads as a
//! value that may be null and may be a bool.

use expect_test::expect;

use super::diagnostics::check_framework;

const PROJECT: &str = r#"
//- /app.php
<?php

interface ContainerInterface
{
    public function getParameter(string $name): array|bool|string|int|float|null;
}

class Repo
{
    public function formats(array $formats): void {}

    public function debugging(bool $on): void {}

    public function run(ContainerInterface $container): void
    {
        $this->formats($container->getParameter('cache_default_bin_backends'));
        $this->debugging($container->getParameter('security.enable_super_user'));
    }
}
"#;

#[test]
fn a_parameters_shape_is_what_the_call_returns() {
    check_framework(
        PROJECT,
        |world| {
            world.contracts_mut().bind_argument(
                "ContainerInterface",
                "getParameter",
                0,
                "",
                "parameter",
            );
            world.contracts_mut().catalog_shape(
                "parameter",
                "cache_default_bin_backends",
                phrost_model::BuiltinTy::Array,
            );
            world.contracts_mut().catalog_shape(
                "parameter",
                "security.enable_super_user",
                phrost_model::BuiltinTy::Bool,
            );
        },
        expect![[r#""#]],
    );
}

#[test]
fn and_without_the_shapes_every_one_of_them_is_reported() {
    // The whole answer arrives from the configuration: a project whose container declares nothing
    // reads the declaration, which lists every kind and a null.
    check_framework(
        PROJECT,
        |world| {
            world.contracts_mut().bind_argument(
                "ContainerInterface",
                "getParameter",
                0,
                "",
                "parameter",
            );
        },
        expect![[r#"
            16:24-16:78 warning [possibly-null-argument] $formats takes array, and this may be null
            17:26-17:80 warning [possibly-null-argument] $on takes bool, and this may be null
        "#]],
    );
}
