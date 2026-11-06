//! Which classes inherit from the one under the cursor.
//!
//! The reverse of every other hierarchy question here: a class knows what it extends and nothing
//! knows what extends it. This tests the per-file half -- given a file and a name, which of that
//! file's classes name it -- because that is where being wrong is silent: the sweep over candidate
//! files would find nothing and look like a project with no implementers.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::goto_implementation;

/// Every implementer of the name at the marker, in the order offered.
///
/// Through the harness's own check, which runs the answer twice -- with one buffer open and with all
/// of them -- and requires the two to agree. That is the invariant this feature could most easily
/// break: it reads the class table, and a version that read a body would answer only about files the
/// editor happens to have open.
fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position(marker);
    let asking = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(asking), |open| {
        let view = IndexView::in_file(world.skeleton(), open, asking);
        let found = goto_implementation(&view, position.offset);
        if found.is_empty() {
            return "(nothing inherits from what is here)\n".to_owned();
        }
        found
            .into_iter()
            .map(|target| {
                format!(
                    "{} {} @ {}\n",
                    target.kind.as_str(),
                    target.name,
                    world.path_of(target.file)
                )
            })
            .collect()
    });

    expect.assert_eq(&rendered);
}

#[test]
fn an_interface_finds_what_implements_it_across_files() {
    check(
        r#"
//- /src/Storage.php
<?php
namespace App;

interface StorageInterface {}

//- /src/Sql.php
<?php
namespace App;

class SqlStorage implements StorageInterface {}

//- /src/Other.php
<?php
namespace Other;

use App\StorageInterface;

class Cached implements StorageInterface {}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(Storage@here@Interface $storage): void {}
}
"#,
        "here",
        expect![[r#"
            class Other\Cached @ src/Other.php
            class App\SqlStorage @ src/Sql.php
        "#]],
    );
}

#[test]
fn a_class_of_the_same_short_name_elsewhere_is_not_an_implementer() {
    // The reason the comparison is on the resolved name and not on the written one: `Repo` means a
    // different class in a different namespace, and a sweep driven by the short name reads both.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

interface Repo {}

//- /src/Other.php
<?php
namespace Other;

interface Repo {}

class Mine implements Repo {}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(Re@here@po $repo): void {}
}
"#,
        "here",
        expect!["(nothing inherits from what is here)\n"],
    );
}

#[test]
fn a_trait_finds_the_classes_that_use_it() {
    // A trait is not a supertype, but goto on one wants its users for the reason goto on an interface
    // wants its implementers: that is where the code it contributes actually runs.
    check(
        r#"
//- /src/Loggable.php
<?php
namespace App;

trait Loggable {}

//- /src/Service.php
<?php
namespace App;

class Service
{
    use Loggable;
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): void
    {
        Logg@here@able::class;
    }
}
"#,
        "here",
        expect![[r#"
            class App\Service @ src/Service.php
        "#]],
    );
}

#[test]
fn a_local_variable_has_no_implementations() {
    check(
        r#"
//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(int $count): void
    {
        $cou@here@nt;
    }
}
"#,
        "here",
        expect!["(nothing inherits from what is here)\n"],
    );
}

#[test]
fn a_method_finds_the_classes_that_override_it() {
    check(
        r#"
//- /src/Storage.php
<?php
namespace App;

interface StorageInterface
{
    public function load(int $id): void;
}

//- /src/Base.php
<?php
namespace App;

abstract class StorageBase implements StorageInterface
{
    public function load(int $id): void {}
}

//- /src/Silent.php
<?php
namespace App;

/// Implements the interface and inherits `load` rather than declaring it.
class Silent extends StorageBase {}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(StorageInterface $storage): void
    {
        $storage->lo@here@ad(1);
    }
}
"#,
        "here",
        expect![[r#"
            method load @ src/Base.php
        "#]],
    );
}

#[test]
fn a_stub_internal_marker_interface_is_not_offered_as_an_implementer() {
    // `__INTERNAL_DO_NOT_USE__StringBackedEnum` really does `implements BackedEnum` in the
    // standard library, which narrows a backed enum's `$value` for the type checker -- but nobody
    // asking "who implements `BackedEnum`" wants it in the answer, any more than they want it in
    // a hover card. See `crate::render::is_stub_internal`.
    check(
        r#"
//- /src/BackedEnum.php
<?php
interface UnitEnum {}
interface BackedEnum extends UnitEnum {}
interface __INTERNAL_DO_NOT_USE__StringBackedEnum extends BackedEnum {}

//- /src/Stage.php
<?php
enum Stage: string implements BackedEnum
{
    case Draft = 'draft';
}

//- /src/Caller.php
<?php
function run(Backed@here@Enum $backed): void {}
"#,
        "here",
        expect![[r#"
            enum Stage @ src/Stage.php
        "#]],
    );
}

#[test]
fn a_subclass_that_only_inherits_the_method_is_not_an_override() {
    // Asked from the declaration's own side this time. Without the check, every subclass answers
    // with the interface's declaration -- which is where an inherited member really is declared, so
    // the location would be right and the list would be nonsense.
    check(
        r#"
//- /src/Storage.php
<?php
namespace App;

interface StorageInterface
{
    public function lo@here@ad(int $id): void;
}

//- /src/Silent.php
<?php
namespace App;

class Silent implements StorageInterface
{
    public function other(): void {}
}
"#,
        "here",
        expect!["(nothing inherits from what is here)\n"],
    );
}

/// Two files declaring one class name, which is a project that does not run and a list that has to
/// be right about it anyway.
///
/// The catalog ranks the claimants and gives one winner, which is the correct answer to "where is
/// `App\Money`" and the wrong one here: the sweep found *two* declarations, one per file, and
/// looking each of them up again by name rendered the winner twice and left the other out. psl's
/// `docs/examples/types/comparison-{comparable,sorting}.php` are exactly that, and the implementers
/// of `Equable` held `comparison-comparable.php` twice.
///
/// Found by `tools/implementers-are-what-is-written.py`, which asks only that a class writing a name
/// in its own header is in that name's implementer list.
#[test]
fn a_name_two_files_declare_is_listed_once_per_file() {
    check(
        r#"
//- /src/Equable.php
<?php
namespace App;

interface Equ@here@able {}

//- /src/first.php
<?php
namespace App;

class Money implements Equable {}

//- /src/second.php
<?php
namespace App;

class Money implements Equable {}
"#,
        "here",
        expect![[r#"
            class App\Money @ src/first.php
            class App\Money @ src/second.php
        "#]],
    );
}
