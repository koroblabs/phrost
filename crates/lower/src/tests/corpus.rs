//! Corpus smoke test over a real PHP checkout.
//!
//! Ignored by default because it needs a checkout this repository does not carry. Run it against
//! phpstorm-stubs, Drupal core, or a Laravel skeleton:
//!
//! ```sh
//! PHROST_CORPUS=/path/to/checkout cargo test -p phrost-lower -- --ignored --nocapture corpus
//! ```
//!
//! This is the main net for a parser upgrade. Unit fixtures cover the constructs someone thought
//! to write down; a real checkout covers the ones nobody did.

use std::{fs, path::PathBuf};

use phrost_model::{FileId, Interner};
use phrost_syntax::ParseSession;

#[test]
#[ignore = "needs PHROST_CORPUS pointing at a PHP checkout"]
fn corpus_lowers_without_panicking() {
    let Ok(root) = std::env::var("PHROST_CORPUS") else {
        panic!("set PHROST_CORPUS to a directory containing PHP files");
    };

    let mut interner = Interner::new();
    let mut files = 0_u32;
    let mut with_errors = 0_u32;
    let mut classes = 0_usize;
    let mut members = 0_usize;

    for path in php_files(PathBuf::from(&root)) {
        let Ok(bytes) = fs::read(&path) else { continue };
        // Lossy on purpose: real checkouts contain files with stray non-UTF-8 bytes, and a strict
        // decode would skip them and hide whatever they declare.
        let text = String::from_utf8_lossy(&bytes);

        let session = ParseSession::new();
        let parsed = session.parse(&text);
        let lowered = crate::lower_file(
            &mut interner,
            FileId::from_raw(files),
            &parsed,
            &crate::KeyedAttributes::default(),
        );

        files += 1;
        if !lowered.errors.is_empty() {
            with_errors += 1;
        }
        let stats = lowered.hir.stats();
        classes += stats.classes;
        members += stats.members;
    }

    println!(
        "corpus {root}: {files} files, {with_errors} with parse errors, \
         {classes} classes, {members} members, interner {:?}",
        interner.stats()
    );
    assert!(files > 0, "no PHP files found under {root}");
}

/// What a real checkout's bodies cost, in nodes and in bytes.
///
/// The number a change to the shape of an expression is judged by. Body IR is lowered only for open
/// buffers, so this is not what bootstrap costs -- it is what an *open file* costs, and it is the one
/// part of a file's HIR that grows with what the code does rather than with what it declares. A node
/// added per element of every array literal is invisible in a fixture and plain here.
///
/// Per file as well as in total, because the total is dominated by generated data tables nobody
/// opens: a country list is thirty thousand array elements and a controller is a few hundred, and it
/// is the second that decides what an editor holds.
///
/// Bodies on purpose, unlike the smoke test above: `lower_file` skips them, so counting there would
/// count nothing.
#[test]
#[ignore = "needs PHROST_CORPUS pointing at a PHP checkout"]
fn corpus_body_ir_size() {
    let Ok(root) = std::env::var("PHROST_CORPUS") else {
        panic!("set PHROST_CORPUS to a directory containing PHP files");
    };

    let mut interner = Interner::new();
    let mut per_file = Vec::new();
    let mut stmts = 0_usize;
    let mut exprs = 0_usize;

    for path in php_files(PathBuf::from(&root)) {
        let Ok(bytes) = fs::read(&path) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let file = u32::try_from(per_file.len()).expect("corpus fits in u32");
        let lowered = crate::lower_text_with_bodies(
            &mut interner,
            FileId::from_raw(file),
            &text,
            &crate::KeyedAttributes::default(),
        );

        let stats = lowered.hir.stats();
        stmts += stats.stmts;
        exprs += stats.exprs;
        per_file.push(stats.exprs);
    }

    per_file.sort_unstable();
    let at = |percent: usize| per_file[per_file.len() * percent / 100];
    let bytes = exprs * size_of::<phrost_hir::body::ExprData>()
        + stmts * size_of::<phrost_hir::body::StmtData>();

    println!(
        "corpus {root}: {} files, {stmts} statements, {exprs} expressions, {} MiB\n\
         expressions per file: p50 {}, p90 {}, p99 {}, max {}\n\
         one expression is {} bytes, one statement {}",
        per_file.len(),
        bytes / (1024 * 1024),
        at(50),
        at(90),
        at(99),
        per_file.last().copied().unwrap_or_default(),
        size_of::<phrost_hir::body::ExprData>(),
        size_of::<phrost_hir::body::StmtData>(),
    );
    assert!(!per_file.is_empty(), "no PHP files found under {root}");
}

/// Every `.php` file under `root`, depth-first.
///
/// A hand-rolled walk rather than a dependency: this is the only place in the crate that touches
/// the filesystem, and it is test-only.
fn php_files(root: PathBuf) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root];

    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "php") {
                found.push(path);
            }
        }
    }

    found.sort();
    found
}
