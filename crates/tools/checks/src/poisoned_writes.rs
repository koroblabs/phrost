//! Who is allowed to take the workspace's write lock.
//!
//! Tokio's `RwLock` does not poison. So a handler that unwinds while holding the write guard
//! releases it and leaves the index readable and half-updated, and every request afterwards is
//! answered from a state no file on disk describes -- which is the one outcome §1.2 forbids:
//! neither a right answer nor a deterministic *not found*.
//!
//! `Session::workspace_write` is the poisoning tokio lacks, and it works by being the only way to
//! get the guard. A write site that takes the lock directly is not wrong-looking, does not fail to
//! compile, and does not fail a test -- it simply is not covered, and the failure it admits is a
//! silent wrong answer hours later. That is exactly the class §1.1 says a rule with no check cannot
//! hold against: the eight sites that exist were routed by hand, and the ninth is the one nobody
//! remembers.
//!
//! A text scan, like its two siblings here, and for the same reason: what is being forbidden is a
//! *spelling*, and the seam has one.

#[cfg(test)]
use std::{fmt::Write as _, fs};

#[cfg(test)]
use crate::workspace_root;

/// What only the owner may write.
#[cfg(test)]
const SEAM: &str = "workspace.write()";

/// The one place allowed to take it, which is where the poisoning lives.
#[cfg(test)]
const OWNER: &str = "crates/lsp/src/session.rs";

#[test]
fn only_one_place_takes_the_workspace_write_lock() {
    let root = workspace_root();
    let mut offenders = Vec::new();
    let mut saw_owner = false;

    for entry in super::walk_rust_files(&root.join("crates")) {
        let path = entry
            .strip_prefix(&root)
            .unwrap_or(&entry)
            .to_string_lossy()
            .replace('\\', "/");
        // This crate names the seam in order to look for it. Tests are *not* exempt: a test that
        // takes the lock directly is a test that does not exercise the poisoning, and the one that
        // matters here asks what a half-updated index answers.
        if path.starts_with("crates/tools/") {
            continue;
        }
        let text = fs::read_to_string(&entry).unwrap_or_default();
        if !text.contains(SEAM) {
            continue;
        }
        if path == OWNER {
            saw_owner = true;
        } else {
            offenders.push(path);
        }
    }

    assert!(
        saw_owner,
        "nothing in {OWNER} takes the write lock, so this check is measuring nothing -- if the \
         accessor moved, move this with it"
    );

    if offenders.is_empty() {
        return;
    }

    let mut report = format!(
        "only {OWNER} may take the workspace's write lock, because tokio's `RwLock` does not \
         poison and `Session::workspace_write` is the poisoning.\n\nThese take it directly, so an \
         unwind there leaves a half-updated index that the next request is answered from:\n\n"
    );
    for path in &offenders {
        let _ = writeln!(report, "  {path}");
    }
    let _ = write!(
        report,
        "\nUse `self.workspace_write().await` instead; it derefs to `Workspace`, so the call site \
         reads the same."
    );
    panic!("{report}");
}
