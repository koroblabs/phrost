//! A member check on a value whose class answers names nothing declares.
//!
//! `__call` already excuses a class from `undefined-method` -- PHP's own way of saying "this answers
//! anything" -- and it is a **class** mechanism. A mocking library hands a caller an *interface*,
//! and an interface declaring `__call` would be a claim about every implementer, so Mockery's
//! `MockInterface` declares a handful of methods and proxies the rest, and PHPUnit splits its fluent
//! builder across two interfaces the concrete class implements together: `method()` returns the one
//! without `with()` on it.
//!
//! Nothing reported any of it while a mock receiver was untyped. The moment one became typed --
//! reading what a wrapper's body forwards did that -- Laravel's test suite gained 76 findings and
//! was already carrying 558 more of the same shape. All 634 are in `tests/` and none in `src/`,
//! which is the shape the rule has: a mock exists nowhere else.
//!
//! Only this layer can say so: a framework provider is installed here, which is also why the
//! fixture materialises a `vendor/mockery/mockery` for the provider to detect.

use expect_test::expect;

use crate::tests::Fixture;

/// What was reported, as a set. See `unused_declarations::reported` for why.
fn reported(fixture: &Fixture) -> Vec<String> {
    let mut lines = fixture.diagnostics_published();
    lines.sort();
    lines.dedup();
    lines
}

/// A mock interface, a real class, and the same unknown call on each.
const A_MOCK_AND_A_REAL_CLASS: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/", "Mockery\\": "vendor/mockery/mockery/library/Mockery/"}}}
//- /vendor/mockery/mockery/library/Mockery/MockInterface.php
<?php
namespace Mockery;

interface MockInterface
{
    public function shouldReceive(string $name): self;
}
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): int {}
}
//- /src/Uses.php
<?php
namespace App;

use Mockery\MockInterface;

class Uses
{
    public function onAMock(MockInterface $mock): void
    {
        $mock->shouldReceive('find');
        $mock->anythingAtAll();
    }

    public function onARealClass(Repo $repo): void
    {
        $repo->find(1);
        $repo->anythingAtAll();
    }
}
"#;

/// The mock is asked nothing; the real class is still told.
#[tokio::test]
async fn a_class_a_framework_says_answers_anything_is_not_asked_about_its_members() {
    let mut fixture = Fixture::new(A_MOCK_AND_A_REAL_CLASS);
    fixture.initialize(false, false).await;
    fixture.initialized().await;
    fixture.open("src/Uses.php").await;

    expect![[r#"
        [
            "Uses.php:17: undefined-method Undefined method App\\Repo::anythingAtAll()",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}
