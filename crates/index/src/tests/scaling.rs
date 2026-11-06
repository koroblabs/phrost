//! A per-file operation must cost the same in a big project as in a small one.
//!
//! This is the shape of the worst performance bug this project has had: `remove_file` scanned every
//! catalog table on every ingest, so bootstrap was quadratic while looking like a loop over files.
//! It cost 1.4 s on a real checkout and was invisible until a batch of 137 files was added at the
//! front of the queue.
//!
//! What makes it catchable is measuring the *same* operation against two project sizes. Absolute
//! timings in a test suite measure the machine and fail on a busy one; a ratio between two
//! measurements taken seconds apart on the same machine measures the algorithm.

use std::time::{Duration, Instant};

use phrost_model::{FileId, SourceOrigin};

use crate::ProjectSkeleton;

/// Classes in the smaller project, and in the larger one.
///
/// Sized so the catalog is big enough for an accidental scan of it to dominate, while the files
/// stay cheap to parse -- the point is to measure index work, and parse cost is the same in both
/// runs only in absolute terms, so any of it in the sample dilutes the signal.
const SMALL: usize = 2_000;
const LARGE: usize = 8_000;

/// Classes per generated file, to reach that many with few files to parse.
const PER_FILE: usize = 50;

/// How many times the probe file is re-indexed per measurement.
const REPEATS: u32 = 200;

/// How much a per-file operation may slow down when the project grows fourfold.
///
/// Calibrated by injecting the bug: a constant-per-file index measures 0.96--1.01x across runs,
/// and re-adding a scan of the catalog inside `retract` measures 2.3x. The threshold sits between
/// those, far enough from 1.0 to absorb a loaded machine and low enough to catch a scan milder than
/// the one that shipped.
const MAX_GROWTH: f64 = 1.5;

#[test]
fn re_indexing_one_file_does_not_get_slower_as_the_project_grows() {
    // Warm up first: the first run pays for allocator growth and for interning the names both runs
    // share, and charging that to whichever project happened to go first would be measuring the
    // order of the test.
    probe_cost(SMALL);

    let small = probe_cost(SMALL);
    let large = probe_cost(LARGE);

    let growth = large.as_secs_f64() / small.as_secs_f64();
    println!("small {small:?}  large {large:?}  growth {growth:.2}x");
    assert!(
        growth < MAX_GROWTH,
        "re-indexing one file cost {small:?} in a {SMALL}-class project and {large:?} in a \
         {LARGE}-class one ({growth:.2}x). A per-file operation is scanning the whole project."
    );
}

/// Average cost of re-indexing one small file, in a project holding `classes` classes.
fn probe_cost(classes: usize) -> Duration {
    let mut skeleton = ProjectSkeleton::new();
    skeleton.set_root("/fixture");

    let mut next_id = 0_u32;
    let mut allocate = move || {
        let id = FileId::from_raw(next_id);
        next_id += 1;
        id
    };

    for chunk in 0..classes.div_ceil(PER_FILE) {
        let path = format!("/fixture/filler_{chunk}.php");
        skeleton.ingest(
            allocate(),
            path.as_ref(),
            SourceOrigin::User,
            &filler(chunk, PER_FILE),
        );
    }
    skeleton.mark_ready();

    // One file, re-indexed the way a keystroke does it. Deliberately tiny: what is being measured
    // is what the index does around the parse, not the parse.
    let probe = allocate();
    let path = std::path::Path::new("/fixture/probe.php");
    let text = "<?php\nnamespace Probe;\nclass Probe { public function run(): void {} }\n";

    let started = Instant::now();
    for _ in 0..REPEATS {
        skeleton.ingest(probe, path, SourceOrigin::User, text);
    }
    started.elapsed() / REPEATS
}

/// A file declaring `count` distinct classes, to fill the catalog cheaply.
fn filler(chunk: usize, count: usize) -> String {
    let mut text = format!("<?php\nnamespace Filler{chunk};\n");
    for index in 0..count {
        text.push_str(&format!(
            "class Filler{index} {{ public function run(): void {{}} }}\n"
        ));
    }
    text
}
