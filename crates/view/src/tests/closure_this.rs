//! `$this` inside a closure the callee binds: `@param-closure-this`.
//!
//! The class the closure is written in is not what runs. Laravel's macros are registered from a
//! service provider and run as whatever they were registered on.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /vendor/support/Macroable.php
<?php
namespace Support;

trait Macroable
{
    /** @param-closure-this static $macro */
    public static function macro(string $name, object $macro): void {}
}

//- /vendor/support/Collection.php
<?php
namespace Support;

class Collection
{
    use Macroable;

    public function count(): int { return 0; }
}

//- /vendor/support/Manager.php
<?php
namespace Support;

class Manager
{
    /** @param-closure-this $this $callback */
    public function extend(string $driver, \Closure $callback): static { return $this; }

    public function createDriver(): void {}
}
"#;

#[test]
fn a_macro_runs_as_the_class_it_was_registered_on() {
    check_view(
        &[
            FIXTURE,
            r#"
//- /src/Provider.php
<?php
namespace App;

use Support\Collection;
use Support\Manager;

class Provider
{
    public function boot(Manager $manager): void
    {
        Collection::macro('size', function () {
            return $this->co@macro@unt();
        });
        $manager->extend('x', function () {
            return array_map(fn ($v) => $this->create@nested@Driver(), []);
        });
        Collection::macro('still', static function () {
            return $this->bo@static@ot();
        });
        $this->bo@own@ot();
    }
}
"#,
        ]
        .concat(),
        &[
            Query::goto("a macro's $this is the class it is registered on", "macro"),
            Query::goto("and a closure inside a bound one captures it", "nested"),
            Query::goto(
                "a static closure is not bound, so nothing changes",
                "static",
            ),
            Query::goto("outside every closure it is the class", "own"),
        ],
        expect![[r#"
            a macro's $this is the class it is registered on
            - method Support\Collection::count @ vendor/support/Collection.php 8:21-8:26
            and a closure inside a bound one captures it
            - method Support\Manager::createDriver @ vendor/support/Manager.php 9:21-9:33
            a static closure is not bound, so nothing changes
            - method App\Provider::boot @ src/Provider.php 9:21-9:25
            outside every closure it is the class
            - method App\Provider::boot @ src/Provider.php 9:21-9:25
        "#]],
    );
}
