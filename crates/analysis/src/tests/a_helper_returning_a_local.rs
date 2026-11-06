//! An untyped helper that builds a value in a local and returns it.
//!
//! `$connection = Mockery::mock(Connection::class); ...; return $connection;` is how a test helper
//! builds a double, and it describes the chain `Mockery::mock` as surely as returning the call
//! would. Read only through a returned *call*, every such helper left what it returned untyped --
//! 495 member accesses on Laravel's own tests. And the `Foo::class` the call was handed travels with
//! the chain, because a `class-string<T>` factory returns a double *of that class*: without it,
//! `$kernel::SUB_REQUEST` on a stubbed kernel read as a constant missing from the stub.

use expect_test::expect;

use crate::tests::{Query, check};

const FIXTURE: &str = r#"
//- /src/Doubles.php
<?php
namespace App;

interface Stub { public function method(string $name): Stub; }
interface Kernel { const SUB_REQUEST = 2; public function handle(): void; }

class Doubles
{
    /**
     * @template T of object
     * @param class-string<T> $class
     * @return Stub&T
     */
    public static function stub(string $class): Stub {}
}

class KernelTest
{
    private function kernel()
    {
        $kernel = Doubles::stub(Kernel::class);
        $kernel->method('handle');
        return $kernel;
    }

    private function twice()
    {
        $kernel = Doubles::stub(Kernel::class);
        $kernel = Doubles::stub(Stub::class);
        return $kernel;
    }

    public function test(): void
    {
        $k@kernel@ = $this->kernel();
        $t@twice@ = $this->twice();
    }
}
"#;

#[test]
fn the_local_a_helper_returns_is_what_its_one_assignment_built() {
    check(
        FIXTURE,
        &[
            Query::hover(
                "the class the call named, beside what the factory declares",
                "kernel",
            ),
            Query::hover("two assignments say nothing", "twice"),
        ],
        expect![[r#"
            the class the call named, beside what the factory declares
            - code: Stub&Kernel $k
            - note: assigned
            two assignments say nothing
            - code: $t
            - note: assigned
            - note: no declared type
        "#]],
    );
}
