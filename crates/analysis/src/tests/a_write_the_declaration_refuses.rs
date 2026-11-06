//! `readonly` and `private(set)`, which say where a property may be written from.
//!
//! Both are an `Error` at runtime rather than a coercion, and neither was checked: a write from
//! outside the declaring class, a second write to one the constructor promoted, and a write to a
//! `private(set)` property all passed silently.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_readonly_property_written_from_outside_and_from_inside() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Point
{
    public function __construct(public readonly int $x) {}

    public function move(): void
    {
        $this->x = 5;
    }
}

class Mover
{
    public function move(Point $point): void
    {
        $point->x = 9;
    }
}
"#,
        expect![[r#"
            10:9-10:17 error [readonly-write] Cannot write readonly App\Point::$x again -- the constructor promoted it
            18:9-18:18 error [readonly-write] Cannot write readonly App\Point::$x from outside the class that declares it
        "#]],
    );
}

/// PHP 8.2's `readonly class` says it once for every property, and a clone is still the same class.
#[test]
fn a_readonly_class_and_a_clone_of_one() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

readonly class Box
{
    public function __construct(public int $size) {}
}

class Resizer
{
    public function grow(Box $box): void
    {
        $copy = clone $box;
        $copy->size = 10;
    }
}
"#,
        expect![[r#"
            14:9-14:20 error [readonly-write] Cannot write readonly App\Box::$size from outside the class that declares it
        "#]],
    );
}

/// PHP 8.4's asymmetric visibility, which is the read half and the write half said separately.
#[test]
fn a_property_whose_write_half_is_private() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class User
{
    public private(set) string $name = 'n';

    public function rename(string $to): void
    {
        $this->name = $to;
    }
}

class Renamer
{
    public function rename(User $user): void
    {
        $user->name = 'x';
    }
}
"#,
        expect![[r#"
            18:9-18:20 error [readonly-write] Cannot write App\User::$name, which declares private(set)
        "#]],
    );
}

/// A readonly property that is *not* promoted is written once from inside, which is how a lazily
/// initialised one is written -- and which path already ran is not a question the declaration
/// answers.
#[test]
fn a_write_inside_the_class_to_an_unpromoted_readonly_is_left_alone() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Lazy
{
    public readonly int $id;

    public function fill(int $id): void
    {
        $this->id = $id;
    }
}
"#,
        expect![""],
    );
}

/// PHP 8.4's third way of saying a property may not be written: a virtual `get` and no `set`.
///
/// Unlike the two above it has no scope in it at all -- `Error: Property A::$name is read-only`
/// comes out for a write from inside the declaring class as much as from outside, which PHP 8.5.10
/// was asked directly rather than argued from the RFC.
#[test]
fn a_property_hooked_with_only_a_get_refuses_every_write() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Page
{
    protected string $raw = '';

    public string $title { get => $this->raw; }

    public function rename(): void
    {
        $this->title = 'inside';
    }
}

class Renamer
{
    public function run(Page $page): void
    {
        $page->title = 'outside';
    }
}
"#,
        expect![[r#"
            12:9-12:21 error [readonly-write] Cannot write App\Page::$title, which is a virtual property with no set hook
            20:9-20:21 error [readonly-write] Cannot write App\Page::$title, which is a virtual property with no set hook
        "#]],
    );
}

/// And a subclass may override the property and add the `set`, after which a write through the
/// parent's type runs -- so the rule steps aside when anything in the project could be that
/// subclass.
///
/// By name, the way every gate of this shape in the index is asked, and narrowed to declarations
/// that could be an override at all: a class with no parent declaring a set-hooked `$title` says
/// nothing about anybody else's.
#[test]
fn a_subclass_that_adds_the_set_hook_takes_the_rule_away() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Page
{
    protected string $raw = '';

    public string $title { get => $this->raw; }
}

class Editable extends Page
{
    public string $title {
        get => $this->raw;
        set { $this->raw = $value; }
    }
}

class Renamer
{
    public function run(Page $page): void
    {
        $page->title = 'runs at runtime';
    }
}
"#,
        expect![""],
    );
}

/// A class with no parent cannot be anybody's override, so its own set-hooked property does not
/// take the rule away from a name it shares.
#[test]
fn an_unrelated_class_sharing_the_name_does_not() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Page
{
    protected string $raw = '';

    public string $title { get => $this->raw; }
}

class Song
{
    private string $stored = '';

    public string $title {
        get => $this->stored;
        set { $this->stored = $value; }
    }
}

class Renamer
{
    public function run(Page $page): void
    {
        $page->title = 'refused';
    }
}
"#,
        expect![[r#"
            25:9-25:21 error [readonly-write] Cannot write App\Page::$title, which is a virtual property with no set hook
        "#]],
    );
}

/// Virtual is the load-bearing word: a hook that uses `$this->propName` makes the property
/// *backed*, and a backed property holds a slot that takes a write.
///
/// Found by the corpora and not by reasoning. The first spelling of this rule reported four writes
/// in Symfony, and the first of them is `ViewEvent`, which caches into a get-only property from
/// its own constructor and from inside the hook -- both of which run. PHP 8.5.10 was asked.
#[test]
fn a_backed_property_with_only_a_get_still_takes_a_write() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Lazy
{
    public ?string $cached {
        get { return $this->cached ??= 'computed'; }
    }

    public function __construct()
    {
        $this->cached = 'seeded';
    }
}

class Filler
{
    public function run(Lazy $lazy): void
    {
        $lazy->cached = 'from outside';
    }
}
"#,
        expect![""],
    );
}

/// And a hook with no body says nothing about backing at all: `public string $n { get; }` in an
/// interface or an abstract class is a requirement, and whoever implements it decides whether the
/// property holds a slot.
#[test]
fn an_abstract_hook_is_not_a_claim_that_the_property_is_virtual() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

interface HasTitle
{
    public string $title { get; }
}

class Writer
{
    public function run(HasTitle $has): void
    {
        $has->title = 'whoever implements it may hold a slot';
    }
}
"#,
        expect![""],
    );
}

/// A *static* property carries a set visibility too, and that half was missed for as long as the
/// static arm existed.
///
/// The arm was right that `readonly` is refused on one -- PHP says `Static property C::$x cannot be
/// readonly` -- and wrong that nothing else applies. Asked of PHP 8.5.10, the table is the one
/// instance properties follow: the declaring class may write a `private(set)` static, a subclass
/// may not, a subclass may write a `protected(set)` one, and nothing outside may.
#[test]
fn a_static_property_with_a_set_scope_refuses_the_same_writes() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Tally
{
    public static private(set) int $n = 0;

    public static protected(set) string $label = '';

    public static function bump(): void
    {
        static::$n = 1;
    }
}

class Kid extends Tally
{
    public static function relabel(): void
    {
        static::$label = 'allowed';
    }

    public static function reset(): void
    {
        static::$n = 0;
    }
}

class FarAway
{
    public function run(): void
    {
        Tally::$n = 5;
        Tally::$label = 'no';
    }
}
"#,
        expect![[r#"
            25:9-25:19 error [readonly-write] Cannot write App\Tally::$n, which declares private(set)
            33:9-33:18 error [readonly-write] Cannot write App\Tally::$n, which declares private(set)
            34:9-34:22 error [readonly-write] Cannot write App\Tally::$label, which declares protected(set)
        "#]],
    );
}
