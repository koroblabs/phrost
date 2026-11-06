//! Where a class is used, when a file gave it another name.
//!
//! `use Illuminate\Support\Collection as BaseCollection;` and then `new BaseCollection()` is a use
//! of `Collection`, and a reader asking where the class is used means that site. A **rename** is the
//! other answer: the `as` line keeps working, so replacing the spelling would break exactly the
//! sites the alias was keeping correct -- see `rename_alias`, which pins that half.
//!
//! One walk answered both and it answered the rename's question, so find-references, the code lens
//! count and the in-file highlight all stopped at the alias. Found by
//! `tools/navigation-roundtrip.py`: eleven sites in one Laravel file where goto answered
//! `Collection` and references at `Collection` did not list them, and four more on Symfony.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{reference_target, references_in, rename_target};

/// Every range each of the two questions answers with, for the marked cursor.
fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position(marker);
    let cursor = world.file_id(&position.path);
    let open = world.open(OpenScope::EveryFile, Some(cursor));
    let at = IndexView::in_file(world.skeleton(), &open, cursor);

    let mut out = String::new();
    for (asked, target) in [
        ("search", reference_target(&at, position.offset)),
        ("rename", rename_target(&at, position.offset)),
    ] {
        let Some(target) = target else {
            let _ = writeln!(out, "{asked} (nothing to search for)");
            continue;
        };
        for file in spec.files() {
            let id = world.file_id(file.path());
            let view = IndexView::in_file(world.skeleton(), &open, id);
            for range in references_in(&view, &target) {
                let _ = writeln!(
                    out,
                    "{asked} {} {} {}",
                    file.path(),
                    world.render_range(id, range),
                    &world.text_of(id)[range]
                );
            }
        }
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

#[test]
fn a_search_lists_the_aliased_uses_and_a_rename_leaves_them_alone() {
    check(
        r#"
//- /src/Collection.php
<?php
namespace App\Support;

class Colle$0ction {}

//- /src/Pivot.php
<?php
namespace App\Database;

use App\Support\Collection as BaseCollection;

class Pivot
{
    public function all(): BaseCollection
    {
        return new BaseCollection();
    }
}

//- /src/Plain.php
<?php
namespace App\Database;

use App\Support\Collection;

class Plain
{
    public function all(): Collection
    {
        return new Collection();
    }
}
"#,
        "0",
        expect![[r#"
            search src/Collection.php 4:7-4:17 Collection
            search src/Pivot.php 4:5-4:27 App\Support\Collection
            search src/Pivot.php 8:28-8:42 BaseCollection
            search src/Pivot.php 10:20-10:34 BaseCollection
            search src/Plain.php 4:5-4:27 App\Support\Collection
            search src/Plain.php 8:28-8:38 Collection
            search src/Plain.php 10:20-10:30 Collection
            rename src/Collection.php 4:7-4:17 Collection
            rename src/Pivot.php 4:5-4:27 App\Support\Collection
            rename src/Plain.php 4:5-4:27 App\Support\Collection
            rename src/Plain.php 8:28-8:38 Collection
            rename src/Plain.php 10:20-10:30 Collection
        "#]],
    );
}

/// At the alias token itself the question is about the name the line binds, and the answer is
/// where this file writes it -- the token included. A search for the class instead left out the very
/// token it was asked at, which is not a use of the class and never appears in the class's list.
#[test]
fn a_search_at_the_alias_token_lists_the_alias() {
    check(
        r#"
//- /src/Collection.php
<?php
namespace App\Support;

class Collection {}

//- /src/Pivot.php
<?php
namespace App\Database;

use App\Support\Collection as Base$0Collection;

class Pivot
{
    public function all(): BaseCollection
    {
        return new BaseCollection();
    }
}
"#,
        "0",
        expect![[r#"
            search src/Pivot.php 4:31-4:45 BaseCollection
            search src/Pivot.php 8:28-8:42 BaseCollection
            search src/Pivot.php 10:20-10:34 BaseCollection
            rename src/Pivot.php 4:31-4:45 BaseCollection
            rename src/Pivot.php 8:28-8:42 BaseCollection
            rename src/Pivot.php 10:20-10:34 BaseCollection
        "#]],
    );
}
