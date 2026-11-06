//! The vendored standard library has to parse with this project's parser.
//!
//! Not an ignored corpus test: these files ship in the repository, so a parser upgrade or a
//! re-vendor that breaks them is a change CI can and should refuse. A stub file that fails to parse
//! does not announce itself — the symbols it declares just quietly stop existing, and every
//! reference to them reads as undeclared code.

use phrost_model::{FileId, Interner};
use phrost_stubs::StubSet;
use phrost_syntax::ParseSession;

#[test]
fn every_stub_parses_and_lowers_without_errors() {
    let cache = tempfile::tempdir().expect("a temporary directory");
    let stubs = StubSet::install_in(cache.path()).expect("installs");

    let mut interner = Interner::new();
    let mut classes = 0_usize;
    let mut members = 0_usize;
    let mut functions = 0_usize;
    let mut broken = Vec::new();

    for (index, path) in stubs.files().iter().enumerate() {
        let text = std::fs::read_to_string(path).expect("stub is readable");
        let session = ParseSession::new();
        let parsed = session.parse(&text);
        let lowered = phrost_lower::lower_file(
            &mut interner,
            FileId::from_raw(u32::try_from(index).expect("few stubs")),
            &parsed,
            &phrost_lower::KeyedAttributes::default(),
        );

        if !lowered.errors.is_empty() {
            broken.push(format!(
                "{}: {}",
                path.file_name().expect("named").to_string_lossy(),
                lowered.errors.len()
            ));
        }
        let stats = lowered.hir.stats();
        classes += stats.classes;
        members += stats.members;
        functions += stats.functions;
    }

    assert!(broken.is_empty(), "stubs with parse errors: {broken:?}");
    // Lower bounds rather than exact counts: the point is that the standard library is really there,
    // and an exact number would have to be updated on every re-vendor for no added confidence.
    assert!(classes > 500, "only {classes} classes");
    assert!(members > 5_000, "only {members} members");
    assert!(functions > 1_000, "only {functions} functions");
    println!("stubs: {classes} classes, {members} members, {functions} functions");
}
