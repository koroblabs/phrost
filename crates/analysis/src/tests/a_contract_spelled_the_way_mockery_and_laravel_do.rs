//! Three spellings of a conditional return that each dropped the whole contract.
//!
//! **An empty-shape test.** Mockery's `shouldReceive(...$methodNames)` is `($methodNames is list{}
//! ? HigherOrderMessage : Expectation)`, and for a variadic parameter the call answers it exactly:
//! anything written there fills it. The test had no reading, so every expectation chain in a
//! Laravel test resolved nothing past `shouldReceive()`.
//!
//! **A branch that is itself a conditional.** Laravel's `app()` is `($abstract is class-string<T> ?
//! T : ($abstract is null ? Application : mixed))`; the inner conditional had no type to lower, so
//! the outer one went too. It is a chain of two tests now, and `app()` settles both.
//!
//! **A template declared only with a prefix.** `@phpstan-template V` does not contain `@template`,
//! and the prefilter every template reader shares looked for that literal -- so a plain `@return V`
//! bound to an array's elements was never read as one.

use expect_test::expect;

use crate::tests::{Query, check};

const FIXTURE: &str = r#"
//- /src/Doubles.php
<?php
namespace App;

class Expectation { public function andReturn(mixed $value): static { return $this; } }
class HigherOrderMessage {}
class Application { public function has(string $id): bool { return true; } }

class Mock
{
    /** @return ($methodNames is list{} ? HigherOrderMessage : Expectation) */
    public function shouldReceive(string ...$methodNames) {}
}

/**
 * @template T of object
 * @param class-string<T>|null $abstract
 * @return ($abstract is class-string<T> ? T : ($abstract is null ? Application : mixed))
 */
function app(?string $abstract = null) {}

/**
 * @phpstan-template V
 * @phpstan-param array<array-key, V> $items
 * @phpstan-return V|null
 */
function first_of(array $items) { return null; }

function run(Mock $mock): void
{
    $a@named@ = $mock->shouldReceive('save');
    $b@none@ = $mock->shouldReceive();
    $c@app@ = app();
    $d@first@ = first_of([new Application()]);
}
"#;

#[test]
fn each_spelling_keeps_its_contract() {
    check(
        FIXTURE,
        &[
            Query::hover("a method named: not empty", "named"),
            Query::hover("nothing written at the variadic position: empty", "none"),
            Query::hover("the inner conditional settles too", "app"),
            Query::hover("a prefixed template binds the elements", "first"),
        ],
        expect![[r#"
            a method named: not empty
            - code: Expectation $a
            - note: assigned
            nothing written at the variadic position: empty
            - code: HigherOrderMessage $b
            - note: assigned
            the inner conditional settles too
            - code: Application $c
            - note: assigned
            a prefixed template binds the elements
            - code: ?Application $d
            - note: assigned
        "#]],
    );
}
