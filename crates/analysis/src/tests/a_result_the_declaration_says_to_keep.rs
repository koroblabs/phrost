//! PHP 8.5's `#[\NoDiscard]`, which says the return value is the point of the call.
//!
//! A warning at runtime rather than an error, with a way out the runtime message names: `(void)`
//! before the call is the author of the *caller* saying the result was meant to go.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_call_written_as_a_statement_throws_the_result_away() {
    check(
        r#"
//- /src/attributes.php
<?php

/** The 8.5 attribute, declared here because the fixture harness installs no stubs. */
final class NoDiscard
{
    public function __construct(public string $reason = '') {}
}

//- /src/app.php
<?php
namespace App;

#[\NoDiscard]
function tryIt(int $n): bool {}

class Lock
{
    #[\NoDiscard('the result says whether it locked')]
    public function tryLock(): bool {}

    public function plain(): bool {}
}

class Uses
{
    public function run(Lock $lock): void
    {
        tryIt(1);
        $lock->tryLock();
        $lock->plain();
    }
}
"#,
        expect![[r#"
            19:9-19:14 warning [discarded-result] The result of App\tryIt() is declared #[\NoDiscard] -- use it, or write `(void)` before the call
            20:16-20:23 warning [discarded-result] The result of App\Lock::tryLock() is declared #[\NoDiscard] -- use it, or write `(void)` before the call
        "#]],
    );
}

/// The three ways a result is not thrown away: cast to `(void)`, kept, or tested.
#[test]
fn a_result_that_is_used_or_deliberately_voided_is_left_alone() {
    check(
        r#"
//- /src/attributes.php
<?php

/** The 8.5 attribute, declared here because the fixture harness installs no stubs. */
final class NoDiscard
{
    public function __construct(public string $reason = '') {}
}

//- /src/app.php
<?php
namespace App;

#[\NoDiscard]
function tryIt(int $n): bool {}

class Uses
{
    public function run(): void
    {
        (void) tryIt(1);
        $ok = tryIt(2);
        if (tryIt(3)) {
            echo $ok;
        }
    }
}
"#,
        expect![""],
    );
}

/// The attribute belongs to the declaration, so a call through a subclass carries it -- and the
/// report names the class that wrote it.
#[test]
fn an_inherited_declaration_carries_the_attribute() {
    check(
        r#"
//- /src/attributes.php
<?php

/** The 8.5 attribute, declared here because the fixture harness installs no stubs. */
final class NoDiscard
{
    public function __construct(public string $reason = '') {}
}

//- /src/app.php
<?php
namespace App;

class Base
{
    #[\NoDiscard]
    public function attempt(): bool {}
}

class Child extends Base {}

class Uses
{
    public function run(Child $child): void
    {
        $child->attempt();
    }
}
"#,
        expect![[r#"
            16:17-16:24 warning [discarded-result] The result of App\Base::attempt() is declared #[\NoDiscard] -- use it, or write `(void)` before the call
        "#]],
    );
}
