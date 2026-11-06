//! Renaming `$id` in `find(int $id)` where callers write `find(id: 5)`.
//!
//! A parameter's name has been part of how a function is called since PHP 8.0, and a call naming
//! one the callee lacks stops with `Unknown named parameter $id`. The rename edited the declaration
//! and the body as a local's would and left every label behind. Labels are matched by the
//! declaration the call reaches: an unrelated class's `find(id:)` is its own parameter.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{references_in, rename_target};

const FIXTURE: &str = r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function __construct(string $table = 'rows') {}

    /**
     * @param int $id The row.
     */
    public function find(int $i@method@d, bool $fresh = false): ?string
    {
        return $fresh ? (string) $id : null;
    }
}

class Other
{
    public function find(int $id): void {}
}

final class Money
{
    public function __construct(
        public readonly int $amo@promoted@unt,
    ) {}
}

function load(string $ke@function@y): string
{
    $each = fn (string $key) => $key;
    return $each($key);
}

//- /src/Client.php
<?php
namespace App;

function go(Repo $repo, Other $other): void
{
    $repo->find(i@label@d: 5);
    $repo->find(1, fresh: true);
    $repo->find(fresh: true, id: 2);
    $other->find(id: 3);
    load(key: 'x');
    new Repo(table: 'users');
    $five = new Money(amo@promoted_label@unt: 5);
    echo $five->amount;
}
"#;

/// For each marker, what a rename is about and every range it edits, across the fixture's files.
fn check(markers: &[&str], expect: Expect) {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build(&spec);
    let mut out = String::new();
    for marker in markers {
        let position = spec.markers().position(marker);
        let cursor = world.file_id(&position.path);
        let open = world.open(OpenScope::EveryFile, Some(cursor));
        let at = IndexView::in_file(world.skeleton(), &open, cursor);
        let target = rename_target(&at, position.offset).expect("something to rename");
        for file in spec.files() {
            let id = world.file_id(file.path());
            let view = IndexView::in_file(world.skeleton(), &open, id);
            for range in references_in(&view, &target) {
                let _ = writeln!(
                    out,
                    "@{marker} {} {} {}",
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

/// A method's parameter and a function's -- and the closure's `$key` inside the function is a
/// different variable that no call names. A promoted parameter is renamed as the property it
/// declares, which is the reading that already existed, and now takes its label along. And a rename
/// asked at a label is the same rename as one asked at the parameter.
#[test]
fn a_parameter_is_renamed_where_calls_name_it() {
    check(
        &["method", "function", "promoted", "label", "promoted_label"],
        expect![[r#"
            @method src/Repo.php 9:19-9:22 $id
            @method src/Repo.php 11:30-11:33 $id
            @method src/Repo.php 13:34-13:37 $id
            @method src/Client.php 6:17-6:19 id
            @method src/Client.php 8:30-8:32 id
            @function src/Repo.php 29:22-29:26 $key
            @function src/Repo.php 32:18-32:22 $key
            @function src/Client.php 10:10-10:13 key
            @promoted src/Repo.php 25:29-25:36 $amount
            @promoted src/Client.php 12:23-12:29 amount
            @promoted src/Client.php 13:17-13:23 amount
            @label src/Repo.php 9:19-9:22 $id
            @label src/Repo.php 11:30-11:33 $id
            @label src/Repo.php 13:34-13:37 $id
            @label src/Client.php 6:17-6:19 id
            @label src/Client.php 8:30-8:32 id
            @promoted_label src/Repo.php 25:29-25:36 $amount
            @promoted_label src/Client.php 12:23-12:29 amount
            @promoted_label src/Client.php 13:17-13:23 amount
        "#]],
    );
}
