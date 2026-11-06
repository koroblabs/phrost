//! Readiness, which is a fact the index states rather than one callers infer.

use std::path::PathBuf;

use phrost_model::{FileId, SourceOrigin};

use crate::{IndexState, ProjectSkeleton};

#[test]
fn a_fresh_index_is_loading_until_it_is_told_otherwise() {
    // The alternative -- inferring readiness from "the catalog has entries" -- is the bug this
    // project already paid for: requests during a background scan saw a partial catalog, so
    // features appeared to switch themselves on one symbol at a time.
    let mut skeleton = ProjectSkeleton::new();
    assert_eq!(skeleton.state(), IndexState::Loading);
    assert!(!skeleton.state().is_ready());

    skeleton.ingest(
        FileId::from_raw(0),
        &PathBuf::from("/fixture/src/A.php"),
        SourceOrigin::User,
        "<?php\nclass A {}\n",
    );
    // Ingesting a file does not make the project ready. Only the caller knows whether every
    // discovered file has been seen.
    assert_eq!(skeleton.state(), IndexState::Loading);

    skeleton.mark_ready();
    assert!(skeleton.state().is_ready());
}

#[test]
fn re_ingesting_one_file_leaves_the_others_alone() {
    // The property that makes an edit cost what the file costs instead of what the project costs.
    let mut skeleton = ProjectSkeleton::new();
    for (raw, path, text) in [
        (0, "src/A.php", "<?php\nnamespace App;\nclass A {}\n"),
        (1, "src/B.php", "<?php\nnamespace App;\nclass B {}\n"),
    ] {
        skeleton.ingest(
            FileId::from_raw(raw),
            &PathBuf::from("/fixture").join(path),
            SourceOrigin::User,
            text,
        );
    }

    let before = skeleton
        .file(FileId::from_raw(1))
        .expect("B is indexed")
        .stamp();

    skeleton.ingest(
        FileId::from_raw(0),
        &PathBuf::from("/fixture/src/A.php"),
        SourceOrigin::User,
        "<?php\nnamespace App;\nclass A2 {}\n",
    );

    assert_eq!(
        skeleton
            .file(FileId::from_raw(1))
            .expect("B still indexed")
            .stamp(),
        before
    );
    assert_eq!(skeleton.file_count(), 2);
}

/// Whose scope a file's top level is, which decides whether a check about its variables may speak.
///
/// Generic and not about any framework: an autoloaded file is reached through composer's
/// `includeFile()`, a *function*, so its top level is local to that call and nobody outside it can
/// read or write those variables. Every other file is reached by an `include` written somewhere, and
/// there the top level belongs to whoever wrote it.
#[test]
fn only_a_file_the_autoloader_loads_keeps_its_top_level_to_itself() {
    let mut built = super::build(
        &super::fixture_files(
            r#"
//- /src/Widget.php
<?php
namespace App;
class Widget {}

//- /src/bootstrap.php
<?php
namespace App;
$stray = 1;

//- /settings.php
<?php
$databases = [];
"#,
        ),
        super::Order::ByPath,
    );
    built
        .skeleton
        .set_source_roots(vec![PathBuf::from("/fixture/src")]);

    let of = |path: &str| {
        let file = built.ids.get(path).expect("the fixture declares the file");
        built.skeleton.file_scope_is_private(file)
    };

    // Under a namespace root and declaring a class-like: the autoloader is what loads it.
    assert!(of("src/Widget.php"));
    // Under the same root and declaring nothing, so it is required by hand and its variables are
    // the caller's.
    assert!(!of("src/bootstrap.php"));
    // Outside the autoloader's territory entirely.
    assert!(!of("settings.php"));
}
