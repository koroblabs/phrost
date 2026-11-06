//! A test closure runs as the test case its suite configured, not the one `test()` declares.
//!
//! Pest's `test()` says `@param-closure-this TestCase` -- PHPUnit's -- and binds the closure to a
//! test case it generates from what `uses()` names: the project's own `Tests\TestCase` for a whole
//! directory from `tests/Pest.php`, and a trait beside it from the test file itself. Read as
//! PHPUnit's class, every helper the project's test case declares was a missing method.

use expect_test::expect;

use super::diagnostics::check_framework;

const SUITE: &str = r#"
//- /vendor/phpunit/TestCase.php
<?php
namespace PHPUnit\Framework;

abstract class TestCase
{
    public function assertTrue(bool $condition): void {}
}

//- /vendor/pest/functions.php
<?php
use PHPUnit\Framework\TestCase;

/** @param-closure-this TestCase $closure */
function test(string $description, ?\Closure $closure = null): void {}

function uses(string ...$classAndTraits): object {}

//- /tests/TestCase.php
<?php
namespace Tests;

abstract class TestCase extends \PHPUnit\Framework\TestCase
{
    public function login(): void {}
}

//- /tests/WithFaker.php
<?php
namespace Tests;

trait WithFaker
{
    protected string $faker = '';
}

//- /tests/Pest.php
<?php
uses(Tests\TestCase::class)->in('Feature');
"#;

fn configured(world: &mut phrost_harness::World) {
    world.set_configured_this(
        "PHPUnit\\Framework\\TestCase",
        &["uses", "pest"],
        &["extend", "use"],
        "in",
    );
}

#[test]
fn a_test_runs_as_the_test_case_its_directory_configures() {
    check_framework(
        &[
            SUITE,
            r#"
//- /tests/Feature/HomeTest.php
<?php
test('home', function () {
    $this->login();
    $this->assertTrue(true);
    $this->logout();
});
"#,
        ]
        .concat(),
        configured,
        expect![[r#"
            5:12-5:18 error [undefined-method] Undefined method Tests\TestCase::logout()
        "#]],
    );
}

/// A trait the file names beside it is part of the class the closure runs as, so what it declares
/// is reachable however it is declared. The control is a protected property of a class that is not
/// the scope.
#[test]
fn a_trait_the_file_configures_is_the_closures_own() {
    check_framework(
        &[
            SUITE,
            r#"
//- /tests/Feature/FakeTest.php
<?php
use Tests\WithFaker;

uses(WithFaker::class);

test('fake', function () {
    $this->assertTrue($this->faker === '');
    $this->assertTrue((new Other())->secret === '');
});

class Other
{
    protected string $secret = '';
}
"#,
        ]
        .concat(),
        configured,
        expect![[r#"
            8:38-8:44 error [inaccessible-member] Cannot access protected property Other::$secret
        "#]],
    );
}

#[test]
fn a_test_no_call_configures_runs_as_the_declared_one() {
    check_framework(
        &[
            SUITE,
            r#"
//- /tests/Unit/MathTest.php
<?php
test('math', function () {
    $this->assertTrue(true);
    $this->nowhere();
});
"#,
        ]
        .concat(),
        configured,
        expect![[r#"
            4:12-4:19 error [undefined-method] Undefined method PHPUnit\Framework\TestCase::nowhere()
        "#]],
    );
}
