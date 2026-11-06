//! `$case->setUp()` written in one subclass, on an instance of another.
//!
//! PHP's rule for `protected` is about the class the member was *declared* in, not the class the
//! value was reached through: anything under that declaration reaches it, which makes two
//! subclasses of one parent able to reach each other's. Reading the object's own class instead
//! reported sixteen calls in one Laravel test file -- and the shape is ordinary, because a test
//! that drives another test case is how a framework tests its own base class.
//!
//! Three rules, and each is needed: the declaring class rather than the object's, for a member the
//! object inherited; the scope's own table, for a member declared above *both* and inherited down
//! two sides; and neither of those for a class outside the hierarchy, which still cannot reach in.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn two_classes_under_one_parent_reach_its_protected_members() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

abstract class Base
{
    protected function boot(): void {}
}

class Subject extends Base {}

class Driver extends Base
{
    public function drive(Subject $other): void
    {
        $other->boot();
    }
}
"#,
        expect![""],
    );
}

/// And a class outside the hierarchy still cannot.
#[test]
fn a_class_outside_the_hierarchy_cannot() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

abstract class Base
{
    protected function boot(): void {}
}

class Subject extends Base {}

class Stranger
{
    public function drive(Subject $other): void
    {
        $other->boot();
    }
}
"#,
        expect![[r#"
            15:17-15:21 error [inaccessible-member] Cannot access protected method App\Subject::boot()
        "#]],
    );
}
