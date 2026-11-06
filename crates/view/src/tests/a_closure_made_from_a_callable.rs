//! `Closure::fromCallable($f)`: a `Closure` that does what invoking `$f` does.
//!
//! The stub says `@return Closure`, which names the class and not the call it wraps, so invoking
//! the result answered `Closure::__invoke(): mixed`. Read as the callable it was given, the way a
//! first-class callable already is.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn invoking_it_gives_what_the_wrapped_callable_returns() {
    check_view(
        r#"
//- /stubs/closure.php  stub
<?php
final class Closure
{
    public static function fromCallable(callable $callback): Closure {}
    public function __invoke(mixed ...$args): mixed {}
}

//- /src/Maker.php
<?php
namespace App;

class Foo {}

class Maker
{
    public function make(int $i): Foo {}

    public function run(): void
    {
        $d@direct@irect = \Closure::fromCallable([$this, 'make'])(1);
        $c = \Closure::fromCallable($this->make(...));
        $v@viavar@ia = $c(2);
    }
}
"#,
        &[
            Query::variable_type("invoked where it is made", "direct"),
            Query::variable_type("invoked through a local", "viavar"),
        ],
        expect![[r#"
            invoked where it is made
            - Foo
            invoked through a local
            - Foo
        "#]],
    );
}
