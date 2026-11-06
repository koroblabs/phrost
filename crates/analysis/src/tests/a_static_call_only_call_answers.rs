//! `Foo::nope()` on a class whose only magic is `__call`.
//!
//! `__call` answers every name a class does not declare -- for a call on an object. A static call
//! written outside the class's hierarchy is answered by `__callStatic` alone, and without one PHP
//! 8.5.10 raises `Call to undefined method Bag::nope()`. Inside the hierarchy the same spelling is a
//! non-static call on `$this` and `__call` runs, which is why the class the call is written in
//! decides.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_static_call_outside_the_hierarchy_is_not_answered_by_call() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Bag
{
    public function __call($method, $arguments) {}
}

class Both
{
    public function __call($method, $arguments) {}

    public static function __callStatic($method, $arguments) {}
}

class Child extends Bag
{
    public function inside(): void
    {
        Bag::nope();
        parent::nope();
    }
}

function outside(): void
{
    Bag::nope();
    Both::nope();
}
"#,
        expect![[r#"
            27:10-27:14 error [undefined-method] Undefined method App\Bag::nope()
        "#]],
    );
}
