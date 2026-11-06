//! Who is allowed to decide what a project is.
//!
//! A project is not just a set of files. It is an autoload map, a set of file extensions, and the
//! contracts a framework contributes -- non-null returns, argument bindings, and the service catalog
//! that makes `\Drupal::service('id')` resolve to a class. All of that is installed in one place,
//! during setup, and anything that builds an index another way gets a checkout where none of it is
//! there.
//!
//! That is not hypothetical. Two command-line subcommands built their own, and so `query` -- the
//! command whose whole purpose is to show what the server would answer -- answered `object` where
//! the server answered the real class. A report of "the server is wrong" was investigated against a
//! tool that was wrong in a different way, which is worse than having no tool.
//!
//! So the seam is consulted in exactly one place, and this test says which.

#[cfg(test)]
use std::{fmt::Write as _, fs};

#[cfg(test)]
use crate::workspace_root;

/// Naming the framework registry means deciding what a project is.
#[cfg(test)]
const SEAM: &str = "phrost_framework::";

/// The one place allowed to ask, and why it is that place.
/// The one place allowed to ask, which is a module rather than a file.
///
/// It was a single file until that file reached 1 200 lines with a 270-line `set_root` in it, and the
/// provider loop moved next door -- so the seam is `workspace/` and its two halves are the same
/// place: `mod.rs` consults the registry and decides the *order*, and `contributions.rs` installs
/// what one provider gives. Widened deliberately, and to a directory rather than to a list, because a
/// list is how a third file joins without anybody deciding to let it.
#[cfg(test)]
const OWNER: &str = "crates/lsp/src/workspace/";

/// Where the answer is applied to an index, so everything downstream inherits it.
#[cfg(test)]
const WHY: &str = "`Workspace::set_root` installs autoload rules, file extensions, and the returns \
                   contracts together, so every index built through it is the same project";

#[test]
fn one_place_decides_what_a_project_is() {
    let root = workspace_root();
    let mut offenders = Vec::new();
    let mut saw_owner = false;

    for entry in super::walk_rust_files(&root.join("crates")) {
        let path = entry
            .strip_prefix(&root)
            .unwrap_or(&entry)
            .to_string_lossy()
            .replace('\\', "/");
        // A framework's own crate is the seam, a test may set one up to check it, and this crate
        // names the seam in order to look for it. The CLI is deliberately *not* exempt: it is where
        // the drift this test exists for happened.
        if path.starts_with("crates/framework/")
            || path.starts_with("crates/tools/")
            || path.contains("/tests/")
            || path.contains("/src/tests/")
            || path.ends_with("/tests.rs")
        {
            continue;
        }
        let text = fs::read_to_string(&entry).unwrap_or_default();
        if !text.contains(SEAM) {
            continue;
        }
        if path.starts_with(OWNER) {
            saw_owner = true;
        } else {
            offenders.push(path);
        }
    }

    assert!(
        saw_owner,
        "nothing under {OWNER} consults the framework registry, so this check is measuring nothing"
    );

    if offenders.is_empty() {
        return;
    }

    let mut report = format!(
        "only {OWNER} may ask which frameworks a project has, because {WHY}.\n\nThese ask it too, \
         so they build a project that is missing whatever the seam contributes:\n\n"
    );
    for path in &offenders {
        let _ = writeln!(report, "  {path}");
    }
    let _ = write!(
        report,
        "\nIndex through `Workspace` (or the CLI's `Sweep`, which wraps it) instead."
    );
    panic!("{report}");
}
