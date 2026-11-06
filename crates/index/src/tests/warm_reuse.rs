//! Whether a warm start keeps a stored row without reading its file, and when it may not.
//!
//! `ProjectSkeleton::ingest_batch` asks two questions of a file it already holds a row for, cheapest
//! first: does the metadata still match what the row was read under, and if not, do the bytes. The
//! first is the one that makes a warm start cheap and the one that could keep a stale row, so each
//! way a file can change is walked through here and the row is checked against what a fresh index
//! says -- not against a count, because a count of "reused" cannot tell trusting the metadata from
//! reading the file.

use std::{
    fs,
    time::{Duration, SystemTime},
};

use phrost_model::{FileId, SourceOrigin};
use phrost_testkit::MaterializedFixture;

use crate::{BatchFile, ProjectSkeleton, Reused, dump_skeleton};

const FIXTURE: &str = r#"
//- /src/One.php
<?php
namespace App;

class One
{
    public function run(): int { return 1; }
}

//- /src/Two.php
<?php
namespace App;

class Two
{
    public function go(): string { return ''; }
}

//- /src/Three.php
<?php
namespace App;

class Three {}
"#;

const FILES: [&str; 3] = ["src/One.php", "src/Three.php", "src/Two.php"];

fn batch(paths: &[std::path::PathBuf]) -> Vec<BatchFile<'_>> {
    paths
        .iter()
        .enumerate()
        .map(|(index, path)| BatchFile {
            file: FileId::from_raw(u32::try_from(index).expect("a small fixture")),
            path,
            origin: SourceOrigin::User,
        })
        .collect()
}

fn render(skeleton: &ProjectSkeleton, fixture: &MaterializedFixture) -> String {
    dump_skeleton(skeleton, |entry| {
        fs::read_to_string(fixture.root().join(skeleton.display_path(entry)))
            .ok()
            .map(|text| phrost_model::LineIndex::new(&text))
    })
}

/// A fresh index of whatever the fixture holds now: the answer a warm start has to match.
fn from_scratch(fixture: &MaterializedFixture, paths: &[std::path::PathBuf]) -> ProjectSkeleton {
    let mut fresh = super::configured(&fixture.root().display().to_string());
    fresh.ingest_batch(&batch(paths), false);
    fresh.mark_ready();
    fresh
}

#[test]
fn an_unchanged_file_is_kept_without_being_read_and_a_changed_one_is_not() {
    let fixture = MaterializedFixture::parse(FIXTURE);
    let paths: Vec<_> = FILES.iter().map(|path| fixture.path(path)).collect();
    let mut skeleton = super::configured(&fixture.root().display().to_string());

    let cold = skeleton.ingest_batch(&batch(&paths), false);
    assert_eq!(cold, Reused::default(), "a cold batch has nothing to reuse");

    // Nothing changed: every row is kept on its metadata alone.
    let warm = skeleton.ingest_batch(&batch(&paths), true);
    assert_eq!(
        warm,
        Reused {
            unread: 3,
            hashed: 0
        }
    );

    // The same bytes with a new modification time -- what `git switch` there and back leaves. The
    // row is kept, but only after the bytes were read to prove it, and the fingerprint moves with
    // the file so the next start does not read it again.
    let later = SystemTime::now() + Duration::from_secs(5);
    fs::File::options()
        .write(true)
        .open(&paths[0])
        .expect("the fixture file")
        .set_modified(later)
        .expect("mtime is settable");
    let touched = skeleton.ingest_batch(&batch(&paths), true);
    assert_eq!(
        touched,
        Reused {
            unread: 2,
            hashed: 1
        }
    );
    let again = skeleton.ingest_batch(&batch(&paths), true);
    assert_eq!(
        again,
        Reused {
            unread: 3,
            hashed: 0
        },
        "a refreshed fingerprint is trusted next time"
    );

    // New bytes: the row is rebuilt, and holds what a fresh index holds. Same length as before on
    // purpose -- the length is the weakest of the three numbers, and a check that only passes when
    // it differs would pass for the wrong reason.
    let text = fs::read_to_string(&paths[1]).expect("the fixture file");
    assert!(text.contains("class Three {}"));
    fs::write(&paths[1], text.replace("class Three {}", "class Trois {}")).expect("writable");
    let changed = skeleton.ingest_batch(&batch(&paths), true);
    assert_eq!(
        changed.total(),
        2,
        "the rewritten file is the one not reused"
    );
    skeleton.mark_ready();
    assert_eq!(
        render(&skeleton, &fixture),
        render(&from_scratch(&fixture, &paths), &fixture),
        "a warm start holds what a fresh index holds"
    );
}

#[test]
fn a_fingerprint_survives_the_cache_and_a_row_from_a_buffer_has_none() {
    let fixture = MaterializedFixture::parse(FIXTURE);
    let paths: Vec<_> = FILES.iter().map(|path| fixture.path(path)).collect();
    let mut skeleton = super::configured(&fixture.root().display().to_string());
    skeleton.ingest_batch(&batch(&paths), false);

    // A row ingested from text rather than from the disk carries no fingerprint: the text may be
    // an editor's unsaved buffer, and the next start has to read the file to know.
    let text = fs::read_to_string(&paths[2]).expect("the fixture file");
    skeleton.ingest(FileId::from_raw(2), &paths[2], SourceOrigin::User, &text);
    assert_eq!(
        skeleton
            .file(FileId::from_raw(2))
            .map(|held| held.fingerprint()),
        Some(None)
    );

    // Through the cache and back, the two fingerprints are still there and the missing one is
    // still missing.
    const BUILD: u64 = 0x5150_5f70_6872_7374;
    let bytes = skeleton.encode(BUILD);
    let mut next = 0_u32;
    let decoded = ProjectSkeleton::decode(&mut bytes.as_slice(), bytes.len() as u64, BUILD, |_| {
        let id = FileId::from_raw(next);
        next += 1;
        id
    })
    .expect("what this build wrote, this build reads");
    let mut restored = super::configured(&fixture.root().display().to_string());
    restored.adopt(decoded);

    let warm = restored.ingest_batch(&batch(&paths), true);
    assert_eq!(
        warm,
        Reused {
            unread: 2,
            hashed: 1
        },
        "two rows trusted on metadata, the buffer's row read to prove it"
    );
}
