//! Two fatals at class load that nothing else here was watching for.
//!
//! `Cannot redeclare C::m()` and `Class C cannot extend final class A`. Both mean the file does not
//! run at all, and the second is only visible to an index: what forbids it is written in another
//! file, and the shape it catches is a parent made `final` while a subclass was already there.
//!
//! Which names collide was asked of PHP 8.5.10 rather than assumed -- three namespaces, methods
//! folded and properties exact; see
//! [`redeclared_members`](phrost_view::IndexView::redeclared_members).

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_name_a_class_declares_twice_is_reported_and_the_legal_pairs_are_not() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class TwiceMethod
{
    public function m(): void {}

    public function m(): void {}
}

class DifferingCase
{
    public function m(): void {}

    public function M(): void {}
}

class PromotedAndDeclared
{
    public int $x = 1;

    public function __construct(public int $x) {}
}

/**
 * A tag is not a declaration -- PHP never sees one.
 *
 * @property string $scheme
 */
class Documented
{
    protected $scheme = null;
}

class LegalPairs
{
    const m = 1;

    public int $n = 1;

    public function m(): void {}

    public function n(): void {}

    public int $M = 2;
}
"#,
        expect![[r#"
            8:21-8:22 error [redeclared-member] App\TwiceMethod already declares m
            15:21-15:22 error [redeclared-member] App\DifferingCase already declares M
            22:44-22:46 error [redeclared-member] App\PromotedAndDeclared already declares $x
        "#]],
    );
}

/// A `final` parent, and the two neighbours that are refused for a different reason and so are not
/// this check's to report.
#[test]
fn extending_a_final_class_is_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

final class Sealed {}

class Open {}

enum Closed { case A; }

interface Contract {}

class Breaker extends Sealed {}

class Fine extends Open {}
"#,
        expect![[r#"
            12:7-12:14 error [sealed-parent] App\Sealed is final and cannot be extended
        "#]],
    );
}
