//! Which classes inherit from a named one, and which of them a bounded list keeps.
//!
//! The bound is the whole of what is interesting here. A framework's base interface has hundreds of
//! implementers and a list is not a place to put them all -- but a flat cap spends its places on
//! whoever the class table reaches first, and the classes a reader came for are their own.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};

use crate::IndexView;

/// Every implementer of `fqn`, with at most `elsewhere` of them from outside the project.
fn check(fixture: &str, fqn: &str, elsewhere: usize, expect: Expect) {
    let world = World::parse(fixture);
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let mut out = String::new();
        for found in view.implementers_of(fqn, elsewhere) {
            let _ = writeln!(
                out,
                "{:?} {} @ {}",
                found.how,
                found.location.fqn,
                world.path_of(found.location.file)
            );
        }
        out
    });

    expect.assert_eq(&rendered);
}

/// A dependency's interface with implementers on both sides of the project boundary.
const BOTH_SIDES: &str = r#"
//- /vendor/acme/log/LoggerInterface.php
<?php
namespace Acme;

interface LoggerInterface {}

//- /vendor/acme/log/FileLogger.php
<?php
namespace Acme;

class FileLogger implements LoggerInterface {}

//- /vendor/acme/log/NullLogger.php
<?php
namespace Acme;

class NullLogger implements LoggerInterface {}

//- /vendor/acme/log/SyslogLogger.php
<?php
namespace Acme;

class SyslogLogger implements LoggerInterface {}

//- /src/AuditLogger.php
<?php
namespace App;

use Acme\LoggerInterface;

class AuditLogger implements LoggerInterface {}

//- /src/QueueLogger.php
<?php
namespace App;

use Acme\LoggerInterface;

class QueueLogger implements LoggerInterface {}

//- /app.php
<?php
namespace App;

class Service {}
"#;

#[test]
fn a_bound_is_spent_on_dependencies_and_never_on_the_projects_own_classes() {
    // One place for everything outside the project, and both of the project's own classes kept.
    // Measured on a real checkout, a flat cap of a hundred held 100 of Drupal's 732 core and
    // contributed subclasses of `UnitTestCase` and dropped 55 of the 155 the project had written
    // itself -- which is the list a person opens a hierarchy to read.
    check(
        BOTH_SIDES,
        "Acme\\LoggerInterface",
        1,
        expect![[r#"
            Implements App\AuditLogger @ src/AuditLogger.php
            Implements App\QueueLogger @ src/QueueLogger.php
            Implements Acme\FileLogger @ vendor/acme/log/FileLogger.php
        "#]],
    );
}

#[test]
fn a_bound_of_nothing_still_answers_with_every_class_the_project_declares() {
    check(
        BOTH_SIDES,
        "Acme\\LoggerInterface",
        0,
        expect![[r#"
            Implements App\AuditLogger @ src/AuditLogger.php
            Implements App\QueueLogger @ src/QueueLogger.php
        "#]],
    );
}
