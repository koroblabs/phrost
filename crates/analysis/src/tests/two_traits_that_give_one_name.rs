//! Two traits giving one class the same method, with nothing to say which wins.
//!
//! `Trait method T2::m has not been applied as C::m, because of collision with T1::m` is a fatal at
//! class load, and the shape it catches is a trait *gaining* a method: two traits composed happily
//! for a year, one grows a `log()`, and every class using both stops loading.
//!
//! Six of the seven rows below are what PHP does rather than what the wording suggests, and each
//! was run against 8.5.10 one line at a time. The one that cost the most: a member the **parent**
//! holds does not enter into it, because a trait overrides an inherited method -- and the table's
//! provenance cannot tell a trait of this class from one a parent used, since both record the
//! declaring trait. Laravel's `UuidUserStub` is that case and was reported until it was fixed.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn what_php_refuses_and_what_it_resolves() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

trait T1
{
    public function m(): void {}
}

trait T2
{
    public function m(): void {}
}

trait Requires
{
    abstract public function n(): void;
}

trait Supplies
{
    public function n(): void {}
}

trait BaseT
{
    public function shared(): void {}
}

trait Via1
{
    use BaseT;
}

trait Via2
{
    use BaseT;
}

class Plain
{
    use T1, T2;
}

class DeclaresItself
{
    use T1, T2;

    public function m(): void {}
}

class Insteadof
{
    use T1, T2 { T1::m insteadof T2; }
}

class AliasOnly
{
    use T1, T2 { T2::m as other; }
}

class RequirementAndBody
{
    use Requires, Supplies;
}

class OneTraitTwoPaths
{
    use Via1, Via2;
}

class InheritsOne extends Plain
{
    use T1;
}
"#,
        expect![[r#"
            39:7-39:12 error [trait-collision] App\T1 and App\T2 both give App\Plain m, and nothing says which
            56:7-56:16 error [trait-collision] App\T1 and App\T2 both give App\AliasOnly m, and nothing says which
        "#]],
    );
}

/// A property composes when the declarations are one declaration, and is a fatal when they differ --
/// from two traits, and from a trait beside the class's own. Each pair was run against 8.5.10. A
/// trait with a constructor is left alone, since `insteadof` on it decides its promoted properties,
/// and so is a `@property` tag, which declares nothing PHP composes.
#[test]
fn a_property_declared_differently_does_not_compose() {
    check(
        r#"
//- /src/Traits.php
<?php
namespace App;

trait Named { public string $name; }
trait Numbered { public int $name; }
trait AlsoNamed { public string $name; }
trait Hidden { protected string $name; }
trait Untyped { public $name; }
trait Built
{
    public function __construct(public int $name) {}
}
/** @property int $name */
trait Documented {}

final class Clash { use Named, Numbered; }
final class Agree { use Named, AlsoNamed; }
final class Shown { use Named, Hidden; }
final class Loose { use Named, Untyped; }
final class Own { use Named; public int $name; }
final class Promoted { use Named, Built; }
final class Tagged { use Documented; public string $name; }
"#,
        expect![[r#"
            16:13-16:18 error [trait-collision] App\Named and App\Numbered declare App\Clash::$name differently, and PHP refuses to compose them
            18:13-18:18 error [trait-collision] App\Named and App\Hidden declare App\Shown::$name differently, and PHP refuses to compose them
            19:13-19:18 error [trait-collision] App\Named and App\Untyped declare App\Loose::$name differently, and PHP refuses to compose them
            20:13-20:16 error [trait-collision] App\Own and App\Named declare App\Own::$name differently, and PHP refuses to compose them
        "#]],
    );
}
