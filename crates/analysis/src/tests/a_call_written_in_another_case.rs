//! A method called under a different case than it was declared with.
//!
//! PHP folds case on a method name and on nothing else a member can be: `$r->FIND()` and
//! `$r->find()` reach one declaration, while `$r->Find` and `$r->find` are two properties. Asked of
//! 8.5.10 rather than read, because the language is inconsistent here on purpose.
//!
//! A search that missed the other spelling was worse than incomplete -- it made a **rename** rewrite
//! some of a method's call sites and leave the rest answering the old name, which is a project that
//! stops running. Found by the round-trip invariant in `tools/navigation-roundtrip.py`: goto at
//! `$this->assertInstanceof()` in Symfony's tests answered PHPUnit's `assertInstanceOf`, and
//! references at that declaration did not list the site it had just come from.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{reference_target, references_in, rename_target};

/// Every reference in every file of the fixture, from the cursor, for both questions.
///
/// Both, because they are answered by two different targets and only one of them is what a rename
/// rewrites -- and it is the rename half that destroys code when it is short.
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
fn every_case_a_method_is_called_in_is_one_reference() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function fi$0nd(int $id): void {}
}

//- /src/Service.php
<?php
namespace App;

class Service
{
    public function run(Repo $repo): void
    {
        $repo->find(1);
        $repo->FIND(2);
        $repo->Find(3);
    }
}
"#,
        "0",
        expect![[r#"
            search src/Repo.php 6:21-6:25 find
            search src/Service.php 8:16-8:20 find
            search src/Service.php 9:16-9:20 FIND
            search src/Service.php 10:16-10:20 Find
            rename src/Repo.php 6:21-6:25 find
            rename src/Service.php 8:16-8:20 find
            rename src/Service.php 9:16-9:20 FIND
            rename src/Service.php 10:16-10:20 Find
        "#]],
    );
}

/// And a property, which PHP does not fold: two spellings are two properties, and listing both
/// would have a rename write over a member it has nothing to do with.
#[test]
fn a_property_is_found_only_under_its_own_spelling() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public int $fo$0und = 1;

    public int $Found = 2;
}

//- /src/Service.php
<?php
namespace App;

class Service
{
    public function run(Repo $repo): int
    {
        return $repo->found + $repo->Found;
    }
}
"#,
        "0",
        expect![[r#"
            search src/Repo.php 6:16-6:22 $found
            search src/Service.php 8:23-8:28 found
            rename src/Repo.php 6:16-6:22 $found
            rename src/Service.php 8:23-8:28 found
        "#]],
    );
}
