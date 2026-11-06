//! Types stored per member, resolved at ingest.
//!
//! The point of these is that a stored type is a *resolved* one: an import, an alias, and a
//! same-namespace reference all end up naming the same class, and the type layer above will never
//! have to look at the imports of the file a member came from.
//!
//! Display shortens class names to their last segment, which is also what makes the resolution
//! visible here: `Repo` in the rendering below is only correct if the alias was followed.

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

/// Every distinct type the project wrote, to show what the table actually holds.
///
/// Counted against an empty skeleton rather than from zero: the table starts with the types the
/// engine's own variables hold -- see `crate::superglobals` -- and those are not the project's.
/// Absolute, this number would move whenever a key is added to `$_SERVER` and the test would read
/// as a regression in sharing.
fn check_type_count(fixture: &str, expect: usize) {
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let engine = crate::ProjectSkeleton::new().types().len();
    assert_eq!(built.skeleton.types().len() - engine, expect);
}

#[test]
fn a_type_reached_three_different_ways_is_one_type() {
    // `Entity\User`, an aliased `Person`, and a fully-qualified `\App\Entity\User` are the same
    // class. If resolution happened at display time instead of at ingest, these would render
    // differently -- and a type layer would see three unrelated classes.
    check_members(
        r#"
//- /src/Entity/User.php
<?php
namespace App\Entity;
class User {}

//- /src/Repo.php
<?php
namespace App;

use App\Entity\User;
use App\Entity\User as Person;

class Repo
{
    public function byImport(): User {}
    public function byAlias(): Person {}
    public function byFqn(): \App\Entity\User {}
}
"#,
        "App\\Repo",
        expect![[r#"
            method byAlias(): User  public  [own]
            method byFqn(): User  public  [own]
            method byImport(): User  public  [own]
        "#]],
    );
}

#[test]
fn a_relative_type_stays_relative() {
    // `static` means the *called* class, so resolving it at ingest would be wrong: a subclass
    // calling an inherited `make()` gets an instance of itself, not of the declaring class.
    check_members(
        r#"
//- /src/Model.php
<?php
namespace App;

class Model
{
    public static function make(): static {}
    public function copy(): self {}
    public function up(): parent {}
}
"#,
        "App\\Model",
        expect![[r#"
            method copy(): self  public  [own]
            method make(): static  public static  [own]
            method up(): parent  public  [own]
        "#]],
    );
}

#[test]
fn composite_and_nullable_types_survive_as_written() {
    check_members(
        r#"
//- /src/Shapes.php
<?php
namespace App;

interface A {}
interface B {}

class Shapes
{
    public function maybe(): ?A {}
    public function either(): A|B|null {}
    public function both(): A&B {}
    public function dnf(): (A&B)|string {}
}
"#,
        "App\\Shapes",
        expect![[r#"
            method both(): A&B  public  [own]
            method dnf(): (A&B)|string  public  [own]
            method either(): A|B|null  public  [own]
            method maybe(): ?A  public  [own]
        "#]],
    );
}

#[test]
fn parameters_carry_their_types_modifiers_and_promotion() {
    check_members(
        r#"
//- /src/Ctor.php
<?php
namespace App;

class Ctor
{
    public function __construct(
        private readonly string $name,
        public int|float $amount = 0,
        ?self $parent = null,
        callable ...$hooks,
    ) {}

    public function fill(array &$target): void {}
}
"#,
        "App\\Ctor",
        expect![[r#"
            method __construct(private readonly string $name, public int|float $amount = 0, ?self $parent = null, callable ...$hooks)  public  [own]
            method fill(array &$target): void  public  [own]
            property int|float $amount  public  [own]
            property string $name  private readonly  [own]
        "#]],
    );
}

#[test]
fn a_property_carries_its_declared_type_and_an_unannotated_one_carries_none() {
    check_members(
        r#"
//- /src/Suit.php
<?php
namespace App;
enum Suit: string { case Hearts = 'H'; }

//- /src/Bag.php
<?php
namespace App;

class Bag
{
    public private(set) ?Suit $suit = null;
    protected static array $all = [];
    public $untyped;
}
"#,
        "App\\Bag",
        expect![[r#"
            property array $all  protected static  [own]
            property ?Suit $suit  public  [own]
            property $untyped  public  [own]
        "#]],
    );
}

#[test]
fn a_typed_constant_and_an_enum_case_carry_a_type() {
    // PHP 8.3 typed class constants. The case has no written type at all -- its type is the enum,
    // which is inference rather than a declaration, so nothing is stored for it here.
    check_members(
        r#"
//- /src/Suit.php
<?php
namespace App;

enum Suit: string
{
    case Hearts = 'H';

    public const string DEFAULT = 'H';
}
"#,
        "App\\Suit",
        expect![[r#"
            const non-falsy-string DEFAULT  public  [own]
            case Hearts  public  [own]
            ?? incomplete: a parent, interface, or trait was missing or cyclic
        "#]],
    );
}

#[test]
fn one_type_written_in_many_files_is_stored_once() {
    // The whole reason the table is affordable. Three files, three `?string` returns, and the
    // project grows the table by *one* entry: the `?string`. The `string` it wraps costs nothing
    // because the engine's own variables already put one there, which is the same sharing seen
    // from the other side.
    check_type_count(
        r#"
//- /src/A.php
<?php
class A { public function a(): ?string {} }

//- /src/B.php
<?php
class B { public function b(): ?string {} }

//- /src/C.php
<?php
class C { public function c(): ?string {} }
"#,
        1,
    );
}
