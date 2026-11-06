//! Where the index's heap goes, and the one way that report can lie.
//!
//! A `Name` is an `Arc<str>` handed out by the interner, and a project has hundreds of thousands of
//! them pointing at a hundred and ninety thousand distinct strings. A breakdown that counted the
//! bytes at every holder would report several times the real figure -- and it would look plausible,
//! which is worse than being obviously wrong. So the interner counts the strings and everyone else
//! counts the pointer, and this file is what says so.

use phrost_model::{FileId, SourceOrigin};

use crate::ProjectSkeleton;

/// Every file declares the same class with the same members, so the set of distinct strings is the
/// same whatever `files` is -- and the number of member rows is not.
///
/// The same fully-qualified name in several files is not a contrivance: it is what a project with
/// two copies of a library looks like, and the catalog records every declarer of it.
fn project(files: usize) -> ProjectSkeleton {
    const CLASS: &str = "<?php
namespace Fixture;

class Thing
{
    public function first(): void {}
    public function second(): void {}
    public function third(): void {}
    public function fourth(): void {}
}
";

    let mut skeleton = ProjectSkeleton::new();
    skeleton.set_root("/fixture");

    for index in 0..files {
        skeleton.ingest(
            FileId::from_raw(index as u32),
            format!("/fixture/thing_{index}.php").as_ref(),
            SourceOrigin::User,
            CLASS,
        );
    }
    skeleton.mark_ready();
    skeleton
}

#[test]
fn the_parts_add_up_to_the_total() {
    let found = project(4).memory();

    assert_eq!(
        found.rows().iter().map(|(_, bytes)| bytes).sum::<usize>(),
        found.total(),
        "a part was added to the breakdown and left out of the total, or the other way round"
    );
    assert!(found.total() > 0, "an indexed project reports no memory");
}

#[test]
fn a_shared_name_is_stored_once_however_many_rows_point_at_it() {
    // The report's one way to lie, and it would look plausible while doing it. Ten times the files
    // means ten times the member rows and exactly the same six distinct strings -- so `members`
    // grows tenfold and `names` does not move.
    let few = project(20).memory();
    let many = project(200).memory();

    assert!(
        many.members > few.members * 5,
        "ten times the files did not cost more member rows: {} then {}",
        few.members,
        many.members
    );
    assert_eq!(
        few.names, many.names,
        "interned names grew with the row count, so a shared string is being counted at every \
         holder instead of once at the interner"
    );
}

#[test]
fn the_catalog_and_the_type_table_stay_small_beside_the_rows() {
    // The shape the memory rule is about. A project's declarations may be large; the maps that
    // index them, and the table that dedupes their types, must not be the reason RSS grows.
    let found = project(200).memory();

    assert!(
        found.catalog < found.members,
        "the catalog ({}) outweighs the member rows ({}) it indexes",
        found.catalog,
        found.members
    );
    assert!(
        found.types < found.members / 4,
        "the type table ({}) is not small beside the rows that point into it ({}), so interning \
         is not doing what it is for",
        found.types,
        found.members
    );
}
