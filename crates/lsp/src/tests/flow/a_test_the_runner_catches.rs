//! `doc-missing-throws` on a class a framework runs and catches everything from.
//!
//! A `@throws` documents a contract for *callers*, and a test method has no caller in the project:
//! PHPUnit invokes each one and catches every `Throwable` -- that catch is not error handling, it
//! is how a failed assertion becomes a failed test. `Assert::assertSame()` is documented
//! `@throws ExpectationFailedException` for exactly that reason.
//!
//! So once the rule read what a *call* declares, it demanded on every test method of a project
//! whose standard puts a docblock on every method a tag describing the test runner's own control
//! flow: 1 841 of 2 655 findings on one real checkout, 1 513 naming a PHPUnit class.
//!
//! Only this layer can say so: a framework provider is installed here and nowhere below, which is
//! also why the fixture has to materialise a `vendor/phpunit/phpunit` for the provider to detect.

use expect_test::expect;

use crate::tests::Fixture;

/// What was reported, as a set. See `unused_declarations::reported` for why.
fn reported(fixture: &Fixture) -> Vec<String> {
    let mut lines = fixture.diagnostics_published();
    lines.sort();
    lines.dedup();
    lines
}

/// A test class, the runner it extends, and an ordinary class beside it that throws the same way.
const A_TEST_AND_A_SERVICE: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/", "PHPUnit\\": "vendor/phpunit/phpunit/src/"}}}
//- /phrost.toml
[lint]
on = ["doc-missing-throws"]
//- /spl.php
<?php
class Exception {}
class RuntimeException extends Exception {}
//- /vendor/phpunit/phpunit/src/Framework/TestCase.php
<?php
namespace PHPUnit\Framework;

class ExpectationFailedException extends \Exception {}

abstract class TestCase
{
    /**
     * @throws ExpectationFailedException
     */
    public function assertSame(mixed $expected, mixed $actual): void {}
}
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * @throws \RuntimeException
     */
    public function find(): string
    {
        throw new \RuntimeException('no');
    }
}
//- /src/Service.php
<?php
namespace App;

class Service
{
    /**
     * Calls something that throws, and is called by this project.
     */
    public function run(Repo $repo): string
    {
        return $repo->find();
    }
}
//- /src/RepoTest.php
<?php
namespace App;

use PHPUnit\Framework\TestCase;

class RepoTest extends TestCase
{
    /**
     * Finds one.
     */
    public function testFind(): void
    {
        $repo = new Repo();
        $this->assertSame('x', $repo->find());
    }
}
"#;

/// The service is told; the test is not, and both throw the same two things.
#[tokio::test]
async fn a_class_the_test_runner_runs_is_not_asked_to_document_what_it_throws() {
    let mut fixture = Fixture::new(A_TEST_AND_A_SERVICE);
    fixture.initialize(false, false).await;
    fixture.initialized().await;
    fixture.open("src/Service.php").await;
    fixture.open("src/RepoTest.php").await;

    expect![[r#"
        [
            "RepoTest.php: none",
            "Service.php:11: doc-missing-throws throws \\RuntimeException, which App\\Repo::find() declares and the docblock does not",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}
