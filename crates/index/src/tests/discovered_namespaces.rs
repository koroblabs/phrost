//! Which classes a framework reaches without anything naming them.
//!
//! Two rules, and they say different things. A *namespace* says where a framework looks --
//! Drupal's plugin managers walk `src/Plugin/` and its test runner walks `Drupal\Tests\`. A
//! *supertype* says what it builds when it gets there: every `ServiceProviderBase` in a module's
//! `src/` root, every `DrushCommands`. Neither subsumes the other, and a class found by one is
//! named by nothing either way.
//!
//! The namespace rule's whole value is that it is a *segment* test rather than a substring one. `App\Plugin\Foo` is found by a
//! plugin manager walking `src/Plugin/`; `App\PluginHelper` is a class somebody wrote and has to
//! name, and calling it reachable would hide it from the only report that would have found it.

use crate::ProjectSkeleton;

fn scanning(segments: &[&str]) -> ProjectSkeleton {
    let mut skeleton = ProjectSkeleton::default();
    skeleton.set_discovered_namespaces(segments.iter().map(|part| (*part).into()).collect());
    skeleton
}

#[test]
fn a_class_in_a_scanned_namespace_needs_nothing_to_name_it() {
    let skeleton = scanning(&["Plugin"]);
    assert!(skeleton.is_discovered("Drupal\\node\\Plugin\\Field\\FieldWidget\\Foo"));
    assert!(skeleton.is_discovered("App\\Plugin\\Foo"));
}

#[test]
fn a_segment_that_merely_starts_the_same_is_not_one() {
    let skeleton = scanning(&["Plugin"]);
    assert!(!skeleton.is_discovered("App\\PluginHelper\\Foo"));
    assert!(!skeleton.is_discovered("App\\Support\\Plugins\\Foo"));
    assert!(!skeleton.is_discovered("App\\Foo"));
}

#[test]
fn the_case_is_ignored_the_way_php_resolves_a_class() {
    assert!(scanning(&["Tests"]).is_discovered("Drupal\\tests\\app_core\\Unit\\FooTest"));
}

#[test]
fn a_project_whose_framework_scans_nothing_reaches_nothing_this_way() {
    assert!(!scanning(&[]).is_discovered("App\\Plugin\\Foo"));
}

/// A skeleton over `fixture`, told which supertypes a framework builds subclasses of.
fn inheriting(supertypes: &[&str], fixture: &str) -> ProjectSkeleton {
    let files = crate::tests::fixture_files(fixture);
    let mut built = crate::tests::build(&files, crate::tests::Order::ByPath);
    built
        .skeleton
        .set_discovered_supertypes(supertypes.iter().map(|name| (*name).into()).collect());
    built.skeleton
}

/// Two levels of base class, an interface, and a class that inherits neither.
const A_HIERARCHY: &str = r#"
//- /vendor/drush/DrushCommands.php
<?php
namespace Drush\Commands;

class DrushCommands {}

//- /src/Base.php
<?php
namespace App;

use Drush\Commands\DrushCommands;

abstract class Base extends DrushCommands {}

//- /src/Policy.php
<?php
namespace App;

class Policy extends Base {}

//- /src/Plain.php
<?php
namespace App;

class Plain {}
"#;

#[test]
fn a_class_extending_a_supertype_a_framework_builds_needs_nothing_to_name_it() {
    let skeleton = inheriting(&["Drush\\Commands\\DrushCommands"], A_HIERARCHY);
    assert!(skeleton.inherits_a_discovered_supertype("App\\Base"));
}

#[test]
fn the_climb_goes_past_the_base_a_project_wrote_to_the_one_a_framework_knows() {
    // What made this worth having: a project writes its own abstract command class and every real
    // command extends *that*, so a rule that only reads the immediate parent finds none of them.
    let skeleton = inheriting(&["Drush\\Commands\\DrushCommands"], A_HIERARCHY);
    assert!(skeleton.inherits_a_discovered_supertype("App\\Policy"));
    assert!(skeleton.inherits_a_discovered_supertype("\\App\\Policy"));
}

#[test]
fn a_class_that_inherits_nothing_a_framework_builds_is_not_one() {
    let skeleton = inheriting(&["Drush\\Commands\\DrushCommands"], A_HIERARCHY);
    assert!(!skeleton.inherits_a_discovered_supertype("App\\Plain"));
    assert!(!skeleton.inherits_a_discovered_supertype("App\\NotThere"));
}

#[test]
fn a_project_whose_framework_names_no_supertype_asks_nothing() {
    assert!(!inheriting(&[], A_HIERARCHY).inherits_a_discovered_supertype("App\\Policy"));
}

#[test]
fn a_cycle_in_the_hierarchy_does_not_hang() {
    let skeleton = inheriting(
        &["Drush\\Commands\\DrushCommands"],
        r#"
//- /src/Left.php
<?php
namespace App;

class Left extends Right {}

//- /src/Right.php
<?php
namespace App;

class Right extends Left {}
"#,
    );
    assert!(!skeleton.inherits_a_discovered_supertype("App\\Left"));
}
