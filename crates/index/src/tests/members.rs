//! Flattening real PHP through the index.
//!
//! The precedence rules themselves are covered in `phrost-sema` against a hand-built hierarchy.
//! What these tests add is the wiring: that parsing, lowering, resolution, and the catalog together
//! feed the flattener a hierarchy that spans files, and that the resolved names line up.

use expect_test::{Expect, expect};
use phrost_sema::{flatten_class, render_member_table};

use super::{Order, build, fixture_files};

fn check_members(fixture: &str, class: &str, expect: Expect) {
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    expect.assert_eq(&render_member_table(
        &flatten_class(&built.skeleton, class),
        built.skeleton.types(),
    ));
}

#[test]
fn a_hierarchy_spanning_three_files_flattens() {
    check_members(
        r#"
//- /src/Base.php
<?php
namespace App;

abstract class Base
{
    public function find(int $id): ?User {}
    private function internal(): void {}
}

//- /src/Timestamps.php
<?php
namespace App;

trait Timestamps
{
    public ?string $createdAt = null;

    public function touch(): void {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Base
{
    use Timestamps;

    public function find(int $id): ?User {}
    public const string VERSION = '1';
}
"#,
        "App\\Repo",
        expect![[r#"
            method find(int $id): ?User  public  [own]
            method touch(): void  public  [trait App\Timestamps]
            property ?string $createdAt  public  [trait App\Timestamps]
            const string VERSION  public  [own]
        "#]],
    );
}

#[test]
fn a_parent_reached_through_an_import_still_resolves() {
    // The catalog lookup and the import resolution have to agree on the fully-qualified name. If
    // they disagree the parent silently vanishes and every inherited member disappears with it.
    check_members(
        r#"
//- /vendor/acme/src/Model.php
<?php
namespace Acme\Orm;

class Model
{
    public function save(): bool {}
}

//- /src/User.php
<?php
namespace App;

use Acme\Orm\Model;

class User extends Model
{
    public function name(): string {}
}
"#,
        "App\\User",
        expect![[r#"
            method name(): string  public  [own]
            method save(): bool  public  [inherited Acme\Orm\Model]
        "#]],
    );
}

#[test]
fn a_trait_conflict_in_real_source_is_reported_with_resolved_names() {
    check_members(
        r#"
//- /src/T.php
<?php
namespace App;

trait Alpha { public function run(): void {} }
trait Beta { public function run(): void {} }

class C
{
    use Alpha, Beta;
}
"#,
        "App\\C",
        expect![[r#"
            method run(): void  public  [trait App\Alpha]
            !! method run from both App\Alpha and App\Beta
        "#]],
    );
}

#[test]
fn insteadof_and_as_survive_the_round_trip_through_the_index() {
    check_members(
        r#"
//- /src/T.php
<?php
namespace App;

trait Alpha { public function run(): void {} }
trait Beta { public function run(): void {} }

class C
{
    use Alpha, Beta {
        Alpha::run insteadof Beta;
        Beta::run as protected legacyRun;
    }
}
"#,
        "App\\C",
        expect![[r#"
            method run(): void (as legacyRun)  protected  [trait App\Beta]
            method run(): void  public  [trait App\Alpha]
        "#]],
    );
}

#[test]
fn promoted_constructor_properties_appear_as_members() {
    check_members(
        r#"
//- /src/C.php
<?php
namespace App;

class C
{
    public function __construct(
        private readonly int $id,
        public ?string $name = null,
    ) {}
}
"#,
        "App\\C",
        expect![[r#"
            method __construct(private readonly int $id, public ?string $name = null)  public  [own]
            property int $id  private readonly  [own]
            property ?string $name  public  [own]
        "#]],
    );
}

#[test]
fn an_uninstalled_parent_leaves_the_table_incomplete() {
    check_members(
        r#"
//- /src/C.php
<?php
namespace App;

class C extends NotInstalled
{
    public function own(): void {}
}
"#,
        "App\\C",
        expect![[r#"
            method own(): void  public  [own]
            ?? incomplete: a parent, interface, or trait was missing or cyclic
        "#]],
    );
}

#[test]
fn a_class_is_found_regardless_of_the_casing_used_to_ask() {
    check_members(
        r#"
//- /src/C.php
<?php
namespace App;
class Repo { public function find(): void {} }
"#,
        "app\\repo",
        expect![[r#"
            method find(): void  public  [own]
        "#]],
    );
}

#[test]
fn enum_cases_and_constants_are_members_too() {
    check_members(
        r#"
//- /src/Suit.php
<?php
namespace App;

enum Suit: string
{
    case Hearts = 'H';
    case Spades = 'S';

    const string DEFAULT = 'H';

    public function color(): string {}
}
"#,
        "App\\Suit",
        expect![[r#"
            method color(): string  public  [own]
            const non-falsy-string DEFAULT  public  [own]
            case Hearts  public  [own]
            case Spades  public  [own]
            ?? incomplete: a parent, interface, or trait was missing or cyclic
        "#]],
    );
}
