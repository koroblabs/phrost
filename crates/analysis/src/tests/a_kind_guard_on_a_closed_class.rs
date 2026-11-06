//! `is_callable` and `is_scalar` on a union holding a class.
//!
//! An object is callable by what its class declares, so `is_callable($h)` could only say "maybe"
//! of a class operand -- and kept it, which Symfony's `HandlersLocator` writes as
//! `if (is_callable($h)) { $h = new HandlerDescriptor($h); }` over a `HandlerDescriptor|callable`:
//! the final class that declares no `__invoke` survived the guard and was then refused as the
//! callable the guard proved. The class answers now, and the same answer serves the judgement.
//!
//! `is_scalar` rules out every object and every array beside the null, which it used to rule out
//! alone. WordPress writes `@param string|WP_Post_Type $post_type` and branches on it that way.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_class_that_cannot_be_the_kind_leaves_the_guarded_branch() {
    check(
        r#"
//- /src/app.php
<?php

// The stubs are not indexed in a fixture; the guards are read by name either way.
function is_callable(mixed $value): bool
{
}

function is_scalar(mixed $value): bool
{
}

final class Descriptor
{
}

final class Named
{
}

function takes_callable(callable $c): void
{
}

function takes_string(string $s): void
{
}

/**
 * @param Descriptor|callable $handler
 * @param string|Named $name
 */
function run($handler, $name): void
{
    if (is_callable($handler)) {
        takes_callable($handler);
    }
    if (is_scalar($name)) {
        takes_string($name);
    }
    takes_string($name);
}
"#,
        expect![[r#"
            40:18-40:23 error [incompatible-argument] $s takes string, and this is string|Named
        "#]],
    );
}
