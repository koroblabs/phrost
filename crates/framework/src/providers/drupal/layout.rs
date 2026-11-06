//! Where a Drupal installation is.
//!
//! Everything else in this provider is relative to the docroot, and composer lets a project choose
//! where that is -- `web/`, `docroot/`, `html/`, or the project root -- through
//! `composer/installers`. So it is probed rather than configured or assumed.

use std::path::{Path, PathBuf};

const DOCROOTS: &[&str] = &["web", "docroot", "html", ""];

/// The file that says "this is Drupal core".
///
/// Chosen because it is the one file that has been at this path for every version that matters, and
/// because finding it also locates the docroot, which everything else is relative to.
const CORE_MARKER: &str = "core/lib/Drupal.php";

/// The Drupal root inside a project, found by looking for core.
pub(super) fn docroot_of(root: &Path) -> Option<PathBuf> {
    DOCROOTS
        .iter()
        .map(|candidate| root.join(candidate))
        .find(|docroot| docroot.join(CORE_MARKER).is_file())
}

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use super::*;

    #[test]
    fn a_docroot_is_found_wherever_composer_put_it() {
        for (label, marker) in [
            ("web", "//- /web/core/lib/Drupal.php\n<?php class Drupal {}"),
            (
                "docroot",
                "//- /docroot/core/lib/Drupal.php\n<?php class Drupal {}",
            ),
            (
                "html",
                "//- /html/core/lib/Drupal.php\n<?php class Drupal {}",
            ),
            (
                "the project root",
                "//- /core/lib/Drupal.php\n<?php class Drupal {}",
            ),
        ] {
            let fixture = MaterializedFixture::parse(marker);
            assert!(
                docroot_of(fixture.root()).is_some(),
                "core under {label} was not found"
            );
        }
    }

    /// The marker is a file and not a directory name, so a project that merely has a `web/` is not
    /// Drupal -- which is most PHP projects.
    #[test]
    fn a_project_without_core_is_not_drupal() {
        let fixture = MaterializedFixture::parse("//- /web/index.php\n<?php");
        assert_eq!(docroot_of(fixture.root()), None);
    }
}
