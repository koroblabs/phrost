//! Structural invariants about the workspace itself, checked as tests.
//!
//! The architecture's boundaries are the crate dependency graph. That only holds if the graph is
//! checked: the previous attempt at this project documented the same boundaries in prose, and
//! still grew a feature crate that reached directly into parser and index internals, because
//! nothing ever failed when someone added a convenient `use`.
//!
//! So the allowed edges live here as data, and adding one is an explicit edit with a reason in
//! the commit rather than a side effect of a refactor.

pub mod dependency_rules;
// A test and the data it checks, and nothing a caller would use.
#[cfg(test)]
mod baseline;
#[cfg(test)]
mod deep_stacks;
#[cfg(test)]
mod no_fallbacks;
mod one_project;
mod poisoned_writes;

use std::path::{Path, PathBuf};

/// Absolute path to the workspace root.
pub fn workspace_root() -> PathBuf {
    // `crates/tools/checks` -> workspace root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("checks crate should live three levels below the workspace root")
        .to_path_buf()
}

/// Every `.rs` file under a directory, recursively, skipping build output.
#[cfg(test)]
fn walk_rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            found.extend(walk_rust_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found
}
