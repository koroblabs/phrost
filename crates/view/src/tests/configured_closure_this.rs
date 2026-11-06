//! `$this` in a closure whose callee's declaration a suite's configuration overrides.
//!
//! Pest's `test()` declares `@param-closure-this TestCase` -- PHPUnit's -- and binds the closure
//! to a test case generated from what `uses()` names, in the file or, with `->in()`, for a
//! directory from `tests/Pest.php`.

use expect_test::expect;

use super::{Query, check_view_ingested_with};

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

//- /tests/Feature/HomeTest.php
<?php
use Tests\WithFaker;

uses(WithFaker::class);

test('home', function () {
    $this->log@login@in();
    $this->fa@faker@ker;
    $this->assert@feature@True(true);
});

//- /tests/Unit/MathTest.php
<?php
test('math', function () {
    $this->assert@unit@True(true);
});
"#;

#[test]
fn a_test_runs_as_the_test_case_its_suite_configures() {
    check_view_ingested_with(
        SUITE,
        &[
            Query::goto("the directory's test case", "login"),
            Query::goto("the file's trait beside it", "faker"),
            Query::goto("and what the test case inherits", "feature"),
            Query::goto(
                "a directory nothing configures keeps the declared one",
                "unit",
            ),
        ],
        |world| {
            world.set_configured_this(
                "PHPUnit\\Framework\\TestCase",
                &["uses", "pest"],
                &["extend", "use"],
                "in",
            );
        },
        expect![[r#"
            the directory's test case
            - method Tests\TestCase::login @ tests/TestCase.php 6:21-6:26
            the file's trait beside it
            - property Tests\WithFaker::faker @ tests/WithFaker.php 6:22-6:28
            and what the test case inherits
            - method PHPUnit\Framework\TestCase::assertTrue @ vendor/phpunit/TestCase.php 6:21-6:31
            a directory nothing configures keeps the declared one
            - method PHPUnit\Framework\TestCase::assertTrue @ vendor/phpunit/TestCase.php 6:21-6:31
        "#]],
    );
}
