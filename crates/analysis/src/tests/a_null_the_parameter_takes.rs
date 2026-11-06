//! A value that may be null, passed where the parameter takes null, is still judged for the rest.
//!
//! The two checks that judge an argument divide the work by sentinel: a null the parameter does not
//! take is `possibly-null-argument`'s, everything else `incompatible-argument`'s. Each asked its own
//! question about the null, so a `?int` reaching a `?string` fell between them -- the first had
//! nothing to say because the null was welcome, the second declined anything null-ish as the
//! first's -- and was silent while an `int` reaching the same parameter was reported.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_nullable_int_reaching_a_nullable_string_is_reported() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

function wants(?string $s): void {}

function run(?int $id): void
{
    wants($id);
}
"#,
        expect![[r#"
            8:11-8:14 error [incompatible-argument] $s takes ?string, and this is ?int
        "#]],
    );
}

/// `string $s = null` takes null through its default and declares a type that does not, so the
/// null is set aside before the judgement rather than handed to it and refused.
#[test]
fn a_parameter_that_takes_null_through_its_default_judges_the_rest() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

function wants(string $s = null): void {}

function run(?string $name, ?int $id): void
{
    wants($name);
    wants($id);
}
"#,
        expect![[r#"
            9:11-9:14 error [incompatible-argument] $s takes string, and this is ?int
        "#]],
    );
}

/// The null half was never the problem and the rest fits, so there is nothing to say: a subclass
/// handed to its parent, null on both sides.
#[test]
fn a_nullable_subclass_reaching_its_nullable_parent_is_silent() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Animal {}
final class Dog extends Animal {}

function wants(?Animal $a): void {}

function run(?Dog $dog): void
{
    wants($dog);
}
"#,
        expect![""],
    );
}
