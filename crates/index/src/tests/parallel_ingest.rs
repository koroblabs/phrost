//! Whether lowering on several threads produces the index that lowering on one produced.
//!
//! Not an equivalent index -- *the* index. `ProjectSkeleton::ingest_batch` reads each file on the
//! worker that will lower it, lowers it against a table that worker owns, and hands the result to
//! one thread to claim. Three things there could differ from the serial path and none of them would
//! announce it: the text a worker read, the origin it lowered under, and a HIR built against an
//! interner that is not the index's.
//!
//! The whole rendered index is compared, because a spot check of one class would pass while another
//! moved. Verified to be able to fail: lowering user code without bodies -- one word -- breaks it.
//!
//! What this deliberately does *not* prove is that claim order is irrelevant. It is, and not because
//! of anything here: `ProjectSkeleton::rank` sorts by origin and path rather than by `FileId`.
//! Reversing the claim order leaves this test green, which is the right answer and worth knowing
//! before somebody reads the ordering in `bulk.rs` as load-bearing for correctness.

use std::collections::BTreeMap;

use phrost_model::{FileId, SourceOrigin};
use phrost_testkit::MaterializedFixture;

use crate::{BatchFile, dump_skeleton};

/// Files chosen for the parts of ingest that differ between the two paths, not for coverage of PHP.
///
/// A class extending one in another file, so the scope has to be rebuilt against the index's own
/// interner rather than the worker's. A contested plugin id, so the catalog has something to rank.
/// A name carrying a derived key, an attribute carrying a positional one, and a dispatching call in
/// a body -- the three facts lowering has to have kept for the claim to find them. Vendor and stub
/// files beside project ones, because origin decides whether bodies are lowered at all.
const FIXTURE: &str = r#"
//- /src/One.php
<?php
namespace App;

use App\Support\Base;

#[Plugin(id: "widget")]
class One extends Base
{
    public function run(string $name): ?Base { return NULL; }
}

//- /src/Shared.php
<?php
namespace App;

class Shared
{
    public const KIND = 'first';
}

//- /src/Support/Base.php
<?php
namespace App\Support;

abstract class Base
{
    protected int $count = 0;

    abstract public function run(string $name): ?Base;
}

//- /src/Hub.php
<?php
namespace A;

class Hub
{
    public function publish(string $event): void {}

    public function announce(int $n, string $event): void {}
}

//- /src/Emit.php
<?php
namespace App;

class Emit
{
    public function fire(\A\Hub $hub): void
    {
        $hub->publish('started');
    }
}

//- /vendor/pkg/Widget.php
<?php
namespace Pkg;

#[Plugin(id: "widget")]
class Widget
{
    public function label(): string { return 'w'; }
}

//- /vendor/pkg/Shared.php
<?php
namespace App;

class Shared
{
    public const KIND = 'second';
}

//- /vendor/pkg/Listener.php
<?php
namespace Pkg;

function listener_shop_open(): void {}

//- /vendor/other/Widget.php
<?php
namespace Other;

#[Plugin(id: "gadget")]
class Widget
{
    public function label(): string { return 'g'; }
}

//- /src/Bag.php
<?php
namespace App;

/**
 * @template TItem of object
 */
class Bag
{
    /** @var array<int, TItem> */
    private array $items = [];

    /**
     * @param TItem $item
     * @return TItem|null
     */
    public function swap(int $slot, object $item, bool $strict = FALSE): ?object
    {
        return NULL;
    }

    public function count(): int { return 0; }
}

//- /stubs/core.php
<?php

function strlen(string $value): int {}
"#;

/// Anything under `vendor/` is a dependency and anything under `stubs/` is the standard library,
/// which is what decides conflict ranking. The same rule the serial builder uses.
fn origin_of(relative: &str) -> SourceOrigin {
    if relative.starts_with("vendor/") {
        SourceOrigin::Vendor
    } else if relative.starts_with("stubs/") {
        SourceOrigin::Stub
    } else {
        SourceOrigin::User
    }
}

#[test]
fn a_batch_lowered_across_threads_is_the_index_one_thread_produced() {
    let fixture = MaterializedFixture::parse(FIXTURE);
    let root = fixture.root().display().to_string();

    let mut relative: Vec<String> = fixture
        .spec()
        .files()
        .iter()
        .filter(|file| file.path().ends_with(".php"))
        .map(|file| file.path().to_owned())
        .collect();
    relative.sort();
    assert!(relative.len() >= 8, "the fixture is too small to reorder");

    let texts: BTreeMap<String, String> = fixture
        .spec()
        .files()
        .iter()
        .filter(|file| file.path().ends_with(".php"))
        .map(|file| (file.path().to_owned(), file.text().to_owned()))
        .collect();

    // One file at a time, from text already in hand -- what every other caller of `ingest` does.
    let mut serial = super::configured(&root);
    for (index, path) in relative.iter().enumerate() {
        let file = FileId::from_raw(u32::try_from(index).expect("a small fixture"));
        serial.ingest(file, &fixture.path(path), origin_of(path), &texts[path]);
    }
    serial.mark_ready();

    // The same files, read from disk and lowered across workers.
    let mut parallel = super::configured(&root);
    let paths: Vec<_> = relative.iter().map(|path| fixture.path(path)).collect();
    let batch: Vec<BatchFile<'_>> = relative
        .iter()
        .zip(&paths)
        .enumerate()
        .map(|(index, (relative, path))| BatchFile {
            file: FileId::from_raw(u32::try_from(index).expect("a small fixture")),
            path,
            origin: origin_of(relative),
        })
        .collect();
    let reused = parallel.ingest_batch(&batch, false);
    parallel.mark_ready();
    assert_eq!(reused.total(), 0, "a cold batch has no stored row to reuse");

    let render = |skeleton: &crate::ProjectSkeleton| {
        dump_skeleton(skeleton, |entry| {
            texts
                .get(skeleton.display_path(entry))
                .map(|text| phrost_model::LineIndex::new(text))
        })
    };
    assert_eq!(
        render(&serial),
        render(&parallel),
        "lowering across threads produced a different index from lowering on one"
    );

    every_stored_name_is_the_interner_s_own(&parallel, &batch);
}

/// Every `Name` the index stores must be the interner's copy, not a clone of the worker's.
///
/// The half a rendering cannot see, and the one this whole design risks. A worker lowers against a
/// table it owns, so the claim has to swap each name for the project's own on the way in; one that
/// is not swapped reads identically -- a `Name` compares by content, so goto, hover and completion
/// are all correct -- and is a second allocation of a string the project has hundreds of thousands
/// of. That is §5's "never duplicate strings", and the measurement that should catch it does the
/// opposite: `lspkit-memsize` counts an interned string once and the `Arc`s pointing at it not at
/// all, so a duplicated name makes `index --memory` report *less*. On a real checkout the report
/// went from 87.9 MB to 82.7 MB, interned names from 208 700 to 29 484, while resident memory rose.
///
/// Identity rather than text, via `lookup_name`, which is the only question that distinguishes the
/// two. It enumerates the fields that hold a name -- there is no walk over the index that does not,
/// and the alternative measured first (comparing the two interners' sizes) is not sound: the serial
/// path also interns every name lowering *sees*, including ones nothing stores. When a row grows a
/// `Name`, it belongs here.
fn every_stored_name_is_the_interner_s_own(
    skeleton: &crate::ProjectSkeleton,
    batch: &[BatchFile<'_>],
) {
    let interner = skeleton.interner();
    let mut checked = 0_usize;
    let mut same = |name: &phrost_model::Name, what: &str| {
        let canonical = interner
            .lookup_name(name.as_str())
            .unwrap_or_else(|| panic!("{what} `{name}` is in no table at all"));
        assert!(
            // By the address of the bytes, which is the only thing that separates the interner's
            // copy from an identical clone. `Name` compares by content everywhere else, on purpose.
            std::ptr::eq(name.as_str().as_ptr(), canonical.as_str().as_ptr()),
            "{what} `{name}` is a second allocation of a string the index already holds"
        );
        checked += 1;
    };

    for entry in batch {
        let Some(file) = skeleton.file(entry.file) else {
            continue;
        };
        for symbol in file.scope().symbols() {
            same(&symbol.fqn, "a declared symbol");
        }
        for class in file.classes() {
            same(class.fqn(), "a class");
            for template in class.templates() {
                same(&template.name, "a template");
            }
            for member in class.members() {
                same(&member.name, "a member");
                for param in member.params.iter() {
                    same(&param.name, "a parameter");
                }
            }
        }
    }
    // A floor rather than an exact count, so adding a file to the fixture does not fail it -- but a
    // floor at all, because every assertion above is inside a loop and a walk that visited nothing
    // would pass in silence. That is the accepted-empty snapshot in another costume.
    assert!(checked >= 40, "the walk checked only {checked} names");
}

#[test]
fn a_second_batch_over_unchanged_files_reuses_every_row() {
    // The warm half. A file's stamp is a hash of its content, so this is also the check that a
    // worker computes the same stamp the claim stored -- if it did not, a warm start would silently
    // re-lower the whole project and still call itself warm.
    let fixture = MaterializedFixture::parse(FIXTURE);
    let root = fixture.root().display().to_string();

    let mut relative: Vec<String> = fixture
        .spec()
        .files()
        .iter()
        .filter(|file| file.path().ends_with(".php"))
        .map(|file| file.path().to_owned())
        .collect();
    relative.sort();

    let mut skeleton = super::configured(&root);
    let paths: Vec<_> = relative.iter().map(|path| fixture.path(path)).collect();
    let batch: Vec<BatchFile<'_>> = relative
        .iter()
        .zip(&paths)
        .enumerate()
        .map(|(index, (relative, path))| BatchFile {
            file: FileId::from_raw(u32::try_from(index).expect("a small fixture")),
            path,
            origin: origin_of(relative),
        })
        .collect();

    assert_eq!(skeleton.ingest_batch(&batch, true).total(), 0);
    assert_eq!(
        skeleton.ingest_batch(&batch, true),
        crate::Reused {
            unread: batch.len(),
            hashed: 0
        },
        "nothing on disk changed, so every row should have been kept without a read"
    );
}
