//! That the standard library shipped at all, and that what shipped is PHP.

use std::fs;

use crate::{PRELUDE, StubSet};

#[test]
fn the_standard_library_is_embedded() {
    // A stub set that quietly became empty would make every standard-library name unresolvable,
    // and the failure would look like a resolution bug rather than a missing asset.
    assert!(PRELUDE.len() > 100, "only {} stubs", PRELUDE.len());
    assert!(
        PRELUDE.iter().any(|(name, _)| *name == "core.php"),
        "the core extension has to be there"
    );
    assert!(
        PRELUDE
            .iter()
            .any(|(_, contents)| contents.contains("function strlen")),
        "strlen is the canary: if it is missing, so is everything else"
    );
}

#[test]
fn every_stub_is_readable_php() {
    let cache = tempfile::tempdir().expect("a temporary directory");
    let installed = StubSet::install_in(cache.path()).expect("installs");

    assert_eq!(installed.len(), PRELUDE.len());
    for path in installed.files() {
        let text = fs::read_to_string(path).expect("stub is readable");
        assert!(text.starts_with("<?php"), "{} is not PHP", path.display());
    }
}
