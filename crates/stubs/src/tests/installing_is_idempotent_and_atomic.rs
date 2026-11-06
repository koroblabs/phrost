//! That a second install reuses the first, and that an interrupted one is never trusted.

use std::fs;

use crate::StubSet;

#[test]
fn installing_twice_reuses_the_same_directory() {
    let cache = tempfile::tempdir().expect("a temporary directory");

    let first = StubSet::install_in(cache.path()).expect("installs");
    let stamp = first.root().expect("installed").join(".complete");
    let written = fs::metadata(&stamp).expect("stamped").modified().ok();

    let second = StubSet::install_in(cache.path()).expect("installs");

    assert_eq!(first.root(), second.root());
    assert_eq!(
        written,
        fs::metadata(&stamp).expect("still stamped").modified().ok(),
        "the second install rewrote a set it should have reused"
    );
}

#[test]
fn an_interrupted_extraction_is_redone() {
    let cache = tempfile::tempdir().expect("a temporary directory");
    let installed = StubSet::install_in(cache.path()).expect("installs");
    let root = installed.root().expect("installed").to_path_buf();

    // What a crash mid-extraction leaves behind: files present, stamp absent.
    fs::remove_file(root.join(".complete")).expect("stamp exists");
    fs::write(root.join("core.php"), "<?php // truncated").expect("writable");

    StubSet::install_in(cache.path()).expect("installs again");

    let core = fs::read_to_string(root.join("core.php")).expect("readable");
    assert!(
        core.contains("function strlen"),
        "a set left without its stamp must be rewritten, not trusted"
    );
}
