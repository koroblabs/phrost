//! The settings every server built on lspkit keeps -- unwinding panics, `unsafe` forbidden for
//! every crate, a committed lockfile -- held by `lspkit_arch::baseline`, where each is explained.
//! phrost names no exception: it writes no `unsafe`, and the crate that does is lspkit's.

use lspkit_arch::{baseline::Baseline, workspace::Workspace};

#[test]
fn the_baseline_holds() {
    let workspace = Workspace::above(env!("CARGO_MANIFEST_DIR"), 3);
    let problems = Baseline::default().violations(&workspace);
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}
