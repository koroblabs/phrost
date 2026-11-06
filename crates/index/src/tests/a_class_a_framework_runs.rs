//! `is_run_by_a_framework`: the second question a provider can answer with a base class.
//!
//! Apart from `inherits_a_discovered_supertype`, which asks whether a framework *builds* the class,
//! because the two say different things about one hierarchy -- and sharing the climb rather than
//! the list is what keeps them from becoming two copies of one walk.

use crate::ProjectSkeleton;

/// A skeleton over `fixture`, told which supertypes a framework runs.
fn running(supertypes: &[&str], fixture: &str) -> ProjectSkeleton {
    let files = crate::tests::fixture_files(fixture);
    let mut built = crate::tests::build(&files, crate::tests::Order::ByPath);
    built
        .skeleton
        .add_run_supertypes(supertypes.iter().map(|name| (*name).into()));
    built.skeleton
}

/// A runner, a project's own base beneath it, and a class that inherits neither.
const A_TEST_HIERARCHY: &str = r#"
//- /vendor/phpunit/TestCase.php
<?php
namespace PHPUnit\Framework;

class TestCase {}

//- /src/UnitTestCase.php
<?php
namespace Drupal\Tests;

use PHPUnit\Framework\TestCase;

abstract class UnitTestCase extends TestCase {}

//- /src/RepoTest.php
<?php
namespace App\Tests;

use Drupal\Tests\UnitTestCase;

final class RepoTest extends UnitTestCase {}

//- /src/Repo.php
<?php
namespace App;

class Repo {}
"#;

#[test]
fn a_class_two_levels_under_the_runner_is_one_the_framework_runs() {
    let skeleton = running(&["PHPUnit\\Framework\\TestCase"], A_TEST_HIERARCHY);

    assert!(skeleton.is_run_by_a_framework("App\\Tests\\RepoTest"));
    assert!(skeleton.is_run_by_a_framework("Drupal\\Tests\\UnitTestCase"));
    // The named class itself, which is what a project extending it directly relies on.
    assert!(skeleton.is_run_by_a_framework("PHPUnit\\Framework\\TestCase"));
    assert!(!skeleton.is_run_by_a_framework("App\\Repo"));
}

/// A project with no test framework installed asks the question and gets a cheap no.
#[test]
fn a_project_whose_frameworks_run_nothing_answers_no_without_climbing() {
    assert!(!running(&[], A_TEST_HIERARCHY).is_run_by_a_framework("App\\Tests\\RepoTest"));
}
