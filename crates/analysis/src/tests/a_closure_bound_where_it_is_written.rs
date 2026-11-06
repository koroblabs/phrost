//! `$this` inside a closure bound in place: `Closure::bind(function () {...}, $kernel)`, and the
//! closure as the receiver of `->bindTo($kernel)` or `->call($kernel)`.
//!
//! No declaration can say what these bind to, because the object is an argument -- so read as the
//! class the closure is written in, every member of the real `$this` was a missing one. Symfony's
//! debug commands reach into the kernel this way, and Laravel reads a registry's handlers with
//! `(fn () => $this->signalHandlers)->call($this->registry)`.

use expect_test::expect;

use crate::tests::{Query, check};

const KERNEL: &str = r#"
//- /src/Kernel.php
<?php
namespace App;

class Kernel
{
    protected array $handlers = [];

    protected function boot(): void {}
}
"#;

#[test]
fn closure_bind_hands_the_closure_its_second_argument() {
    check(
        &format!(
            "{KERNEL}{}",
            r#"
//- /src/Command.php
<?php
namespace App;

class Command
{
    public function run(Kernel $kernel): void
    {
        $boot = \Closure::bind(function () {
            $this->bo@boot@ot();
        }, $kernel, Kernel::class);
        $boot();
    }
}
"#
        ),
        &[Query::goto("the kernel's method", "boot")],
        expect![[r#"
            the kernel's method
            - method App\Kernel::boot @ src/Kernel.php 8:24-8:28 (full 8:5-8:39)
        "#]],
    );
}

#[test]
fn call_and_bind_to_on_the_closure_itself_hand_it_their_first() {
    check(
        &format!(
            "{KERNEL}{}",
            r#"
//- /src/Command.php
<?php
namespace App;

class Command
{
    public function run(Kernel $kernel): array
    {
        $bound = (function () {
            $this->bo@boot@ot();
        })->bindTo($kernel, Kernel::class);
        $bound();
        return (fn () => $this->hand@handlers@lers)->call($kernel);
    }
}
"#
        ),
        &[
            Query::goto("bound by bindTo", "boot"),
            Query::goto("bound by call", "handlers"),
        ],
        expect![[r#"
            bound by bindTo
            - method App\Kernel::boot @ src/Kernel.php 8:24-8:28 (full 8:5-8:39)
            bound by call
            - property App\Kernel::handlers @ src/Kernel.php 6:21-6:30 (full 6:5-6:36)
        "#]],
    );
}

#[test]
fn a_method_merely_named_bind_binds_nothing() {
    // Laravel's router hands its closure over as the *second* argument of a `bind` of its own, and
    // the closure runs as whatever the router decides -- nothing this call says.
    check(
        r#"
//- /src/Routes.php
<?php
namespace App;

class Router
{
    public function bind(string $key, \Closure $resolve): void {}
}

class Routes
{
    public function own(): int { return 1; }

    public function map(Router $router): void
    {
        $router->bind('post', function () {
            return $this->o@own@wn();
        });
    }
}
"#,
        &[Query::goto("still the class it is written in", "own")],
        expect![[r#"
            still the class it is written in
            - method App\Routes::own @ src/Routes.php 11:21-11:24 (full 11:5-11:45)
        "#]],
    );
}
