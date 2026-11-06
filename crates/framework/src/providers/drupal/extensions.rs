//! Which namespaces Drupal registers at runtime, and where.
//!
//! Composer's autoload map covers `Drupal\Core\` and `Drupal\Component\` and stops. Every installed
//! module registers `Drupal\<machine_name>\` from its `*.info.yml` when the kernel boots, and core's
//! `tests/bootstrap.php` registers seven more. On a real site that is thousands of files and most of
//! the code a developer actually reads.

use std::path::{Path, PathBuf};

use crate::rule::AutoloadRule;

/// One PSR-4 rule per installed extension, plus one for its tests.
///
/// `<dir>/<name>.info.yml` with a sibling `src/` means `Drupal\<name>\` lives there. A sibling
/// `tests/src/` means `Drupal\Tests\<name>\` does — a separate namespace in Drupal, and worth
/// registering because test classes are read and navigated like any other.
pub(super) fn autoload_rules(docroot: &Path, descriptors: &[PathBuf]) -> Vec<AutoloadRule> {
    let mut rules = Vec::new();
    for info in descriptors {
        let Some(directory) = info.parent() else {
            continue;
        };
        let Some(name) = machine_name(info) else {
            continue;
        };

        if directory.join("src").is_dir() {
            rules.push(AutoloadRule {
                prefix: format!("Drupal\\{name}\\"),
                dir: directory.join("src"),
            });
        }
        if directory.join("tests/src").is_dir() {
            rules.push(AutoloadRule {
                prefix: format!("Drupal\\Tests\\{name}\\"),
                dir: directory.join("tests/src"),
            });
        }
    }

    rules.extend(core_test_rules(docroot));
    rules.sort();
    rules.dedup();
    rules
}

/// Core's own test namespaces, one per directory under `core/tests/Drupal`.
///
/// `core/tests/bootstrap.php` registers seven of these at runtime -- `Drupal\Tests`,
/// `Drupal\KernelTests`, `Drupal\TestTools` and the rest -- and composer's map has none of them.
/// Without them `Drupal\Tests\UnitTestCase` is not indexed, and since nearly every test in a
/// Drupal project extends it, the whole hierarchy above every test class is unreachable: every
/// `assertSame` and `createMock` in the project resolves to nothing.
///
/// Read from the directory listing rather than from a list written here, because the set has grown
/// between Drupal versions and a stale list fails the same silent way.
fn core_test_rules(docroot: &Path) -> Vec<AutoloadRule> {
    let base = docroot.join("core/tests/Drupal");
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            Some(AutoloadRule {
                prefix: format!("Drupal\\{name}\\"),
                dir: entry.path(),
            })
        })
        .collect()
}

/// The machine name an `*.info.yml` declares, which is its filename stem.
pub(super) fn machine_name(info: &Path) -> Option<&str> {
    info.file_name()?.to_str()?.strip_suffix(".info.yml")
}

/// Whether a file is an extension descriptor. The walk that finds them is `Survey::of`.
pub(super) fn is_descriptor(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".info.yml"))
}

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use crate::{providers::drupal::Drupal, registry::FrameworkProvider};

    /// A Drupal project as composer builds one: core under a docroot, a contrib module, a custom
    /// module with tests, and a non-extension directory that must not produce a rule.
    const PROJECT: &str = r#"
//- /composer.json
{}

//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/core/tests/Drupal/Tests/UnitTestCase.php
<?php namespace Drupal\Tests; class UnitTestCase {}

//- /docroot/core/tests/Drupal/KernelTests/KernelTestBase.php
<?php namespace Drupal\KernelTests; class KernelTestBase {}

//- /docroot/core/modules/node/node.info.yml
name: Node

//- /docroot/core/modules/node/src/NodeStorage.php
<?php namespace Drupal\node; class NodeStorage {}

//- /docroot/modules/contrib/token/token.info.yml
name: Token

//- /docroot/modules/contrib/token/src/Token.php
<?php namespace Drupal\token; class Token {}

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine

//- /docroot/modules/custom/mine/src/Service.php
<?php namespace Drupal\mine; class Service {}

//- /docroot/modules/custom/mine/tests/src/Unit/ServiceTest.php
<?php namespace Drupal\Tests\mine\Unit; class ServiceTest {}

//- /docroot/libraries/thing/readme.txt
not an extension
"#;

    fn rules(fixture: &MaterializedFixture) -> Vec<String> {
        Drupal
            .project(fixture.root())
            .autoload_rules()
            .into_iter()
            .map(|rule| {
                format!(
                    "{} => {}",
                    rule.prefix,
                    rule.dir
                        .strip_prefix(fixture.root())
                        .expect("under root")
                        .to_string_lossy()
                        .replace('\\', "/")
                )
            })
            .collect()
    }

    #[test]
    fn every_installed_extension_gets_a_namespace() {
        // None of these are in composer's autoload map: Drupal registers them at runtime from the
        // info files. Without this, most of a Drupal project's classes resolve to nothing.
        let fixture = MaterializedFixture::parse(PROJECT);

        assert_eq!(
            rules(&fixture),
            [
                "Drupal\\KernelTests\\ => docroot/core/tests/Drupal/KernelTests",
                "Drupal\\Tests\\ => docroot/core/tests/Drupal/Tests",
                "Drupal\\Tests\\mine\\ => docroot/modules/custom/mine/tests/src",
                "Drupal\\mine\\ => docroot/modules/custom/mine/src",
                "Drupal\\node\\ => docroot/core/modules/node/src",
                "Drupal\\token\\ => docroot/modules/contrib/token/src",
            ]
        );
    }

    #[test]
    fn drupal_is_detected_by_finding_core_not_by_guessing_a_layout() {
        // The docroot is a project choice -- `web`, `docroot`, or the root -- so it is probed.
        let fixture = MaterializedFixture::parse(PROJECT);
        assert!(Drupal.detect(fixture.root()));

        let not_drupal = MaterializedFixture::parse(
            r#"
//- /composer.json
{}

//- /src/App.php
<?php class App {}
"#,
        );
        assert!(!Drupal.detect(not_drupal.root()));
        assert!(
            Drupal
                .project(not_drupal.root())
                .autoload_rules()
                .is_empty()
        );
    }

    #[test]
    fn a_web_docroot_works_the_same_as_a_docroot_one() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /web/core/lib/Drupal.php
<?php class Drupal {}

//- /web/modules/custom/mine/mine.info.yml
name: Mine

//- /web/modules/custom/mine/src/Service.php
<?php namespace Drupal\mine; class Service {}
"#,
        );

        assert!(Drupal.detect(fixture.root()));
        assert_eq!(
            rules(&fixture),
            ["Drupal\\mine\\ => web/modules/custom/mine/src"]
        );
    }

    #[test]
    fn a_directory_without_a_src_produces_no_rule() {
        // A profile or a theme may declare itself and hold no PHP classes at all. Registering a
        // namespace for a directory that does not exist would make every lookup under it fail
        // slowly instead of quickly.
        let fixture = MaterializedFixture::parse(
            r#"
//- /core/lib/Drupal.php
<?php class Drupal {}

//- /themes/custom/plain/plain.info.yml
name: Plain
"#,
        );

        assert!(Drupal.detect(fixture.root()));
        assert!(rules(&fixture).is_empty());
    }

    #[test]
    fn cores_own_test_namespaces_are_registered() {
        // Registered by `core/tests/bootstrap.php` at runtime and absent from composer's map. Nearly
        // every test in a Drupal project extends `Drupal\Tests\UnitTestCase`, so without these the
        // hierarchy above every test class is unreachable and every contract in it resolves to
        // nothing.
        let fixture = MaterializedFixture::parse(
            r#"
//- /composer.json
{}

//- /core/lib/Drupal.php
<?php class Drupal {}

//- /core/tests/Drupal/Tests/UnitTestCase.php
<?php namespace Drupal\Tests; class UnitTestCase {}

//- /core/tests/Drupal/TestTools/Random.php
<?php namespace Drupal\TestTools; class Random {}
"#,
        );

        assert_eq!(
            rules(&fixture),
            [
                "Drupal\\TestTools\\ => core/tests/Drupal/TestTools",
                "Drupal\\Tests\\ => core/tests/Drupal/Tests",
            ]
        );
    }
}
