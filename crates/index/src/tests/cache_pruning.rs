//! Clearing out cached indexes nobody can use.
//!
//! A cache keyed by project root never forgets a project, and for a long time nothing deleted one:
//! the directory grew by an entry for every root ever opened. A harness that indexes a throwaway
//! project makes that pathological — seven hundred entries and most of a gigabyte, for roots that
//! were deleted the same second.

use std::{fs, path::Path};

use crate::{ProjectSkeleton, cache};

/// A cache entry for `root`, written by `build`.
fn write_entry(directory: &Path, name: &str, root: Option<&Path>, build: u64) {
    let mut skeleton = ProjectSkeleton::new();
    if let Some(root) = root {
        skeleton.set_root(root);
    }
    let path = directory.join(format!("{name}.phrost"));
    cache::store(&path, |sink| skeleton.write_to(sink, build)).expect("write the entry");
}

fn names(directory: &Path) -> Vec<String> {
    let mut found = fs::read_dir(directory)
        .expect("read the directory")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    found.sort();
    found
}

#[test]
fn an_entry_this_build_cannot_read_is_removed() {
    let home = tempfile::tempdir().expect("a directory");
    let project = home.path().join("project");
    fs::create_dir(&project).expect("a project");

    write_entry(home.path(), "current", Some(&project), 7);
    write_entry(home.path(), "older", Some(&project), 6);

    let pruned = cache::prune(home.path(), 7, 8);

    assert_eq!(pruned.removed, 1);
    assert_eq!(names(home.path()), ["current.phrost", "project"]);
}

#[test]
fn an_entry_whose_project_is_gone_is_removed() {
    let home = tempfile::tempdir().expect("a directory");
    let project = home.path().join("kept");
    fs::create_dir(&project).expect("a project");

    write_entry(home.path(), "here", Some(&project), 7);
    write_entry(home.path(), "deleted", Some(&home.path().join("gone")), 7);

    let pruned = cache::prune(home.path(), 7, 8);

    assert_eq!(pruned.removed, 1);
    assert_eq!(names(home.path()), ["here.phrost", "kept"]);
}

#[test]
fn an_entry_recording_no_root_is_kept() {
    // It says nothing either way, and deleting on no evidence is how a cache learns to throw away
    // work somebody wanted.
    let home = tempfile::tempdir().expect("a directory");
    write_entry(home.path(), "rootless", None, 7);

    let pruned = cache::prune(home.path(), 7, 8);

    assert_eq!(pruned.removed, 0);
    assert_eq!(names(home.path()), ["rootless.phrost"]);
}

#[test]
fn past_the_limit_the_least_recently_used_go() {
    let home = tempfile::tempdir().expect("a directory");
    let project = home.path().join("project");
    fs::create_dir(&project).expect("a project");

    for index in 0..5 {
        write_entry(home.path(), &format!("entry{index}"), Some(&project), 7);
    }

    let pruned = cache::prune(home.path(), 7, 2);

    assert_eq!(pruned.removed, 3);
    assert_eq!(pruned.kept, 2);
    assert!(pruned.freed > 0);
}

#[test]
fn a_file_that_is_not_an_entry_is_left_alone() {
    let home = tempfile::tempdir().expect("a directory");
    fs::write(home.path().join("notes.txt"), b"mine").expect("a file");

    let pruned = cache::prune(home.path(), 7, 8);

    assert_eq!(pruned.removed, 0);
    assert_eq!(names(home.path()), ["notes.txt"]);
}
