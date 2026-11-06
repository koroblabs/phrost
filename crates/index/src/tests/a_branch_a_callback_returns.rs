//! A conditional return whose branch is a template only a callable parameter's return binds.
//!
//! Laravel's `optional()`: `($callback is null ? Optional : ($value is null ? null : TReturn))`
//! beside `(callable(TValue): TReturn)|null`. The branch was refused as a template nothing binds,
//! and the whole contract with it -- including the branch every call without a callback settles.
//! Held at the index because the contract is stored, and both invariants compare the dump.

use expect_test::expect;

use super::check_index;

#[test]
fn a_template_a_callback_returns_is_a_branch() {
    check_index(
        r#"
//- /src/Box.php
<?php
namespace App;

class Optional {}

class Box
{
    /**
     * @template TValue
     * @template TReturn
     * @param TValue $value
     * @param (callable(TValue): TReturn)|null $callback
     * @return ($callback is null ? Optional : TReturn)
     */
    public function optional($value, ?callable $callback = null) {}
}
"#,
        expect![[r#"
            state ready
            file src/Box.php [User]
              class App\Optional @ 4:7-4:15
              class App\Box @ 6:7-6:10
              App\Optional is class
              App\Box is class
              App\Box::method optional($value, ?callable $callback = null): mixed  public $callback calls(\App\TValue) returns, if argument 1 is null, \App\Optional and otherwise what the callable at 1 returns by default true  @ 15:21-15:29 (full 15:5-15:68)
            catalog
              [class] app\box -> src/Box.php (App\Box)
              [class] app\optional -> src/Box.php (App\Optional)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}
