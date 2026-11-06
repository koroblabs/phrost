//! `doc-missing-throws`: a throw the body catches where it stands is not thrown out of it.
//!
//! A docblock says what escapes a declaration. `try { throw new X(); } catch (X $e) {}` escapes
//! nothing, so a `@throws X` demanded for it would document something that cannot happen -- and the
//! rule reported every one of them, which matters because wrapping a throwing call in a handler is
//! how PHP code deals with one.
//!
//! The span that guards is the `try` block **alone**. A throw in a `catch` body is not caught by the
//! clause it is written in, and one in `finally` is caught by nothing -- both sit outside it, which
//! is what makes a range test exact rather than approximate.

use expect_test::expect;

use super::diagnostics::check_asked_for;

/// Seven methods, one fixture: four a handler covers and three it does not. Together rather than
/// apart because what has to be pinned is the *boundary*, and a test that only showed the silent
/// half would pass just as well with the check switched off.
#[test]
fn a_throw_is_reported_only_where_no_surrounding_catch_covers_it() {
    check_asked_for(
        r#"
//- /spl.php
<?php
class Exception {}
class RuntimeException extends Exception {}
class LogicException extends Exception {}
class TypeError {}

//- /app.php
<?php
namespace App;

class Repo
{
    /**
     * Caught by its own class, so nothing leaves.
     */
    public function handled(): void
    {
        try {
            throw new \RuntimeException('no');
        }
        catch (\RuntimeException $e) {
        }
    }

    /**
     * Caught by a parent, which is how a handler is usually written.
     */
    public function byParent(): void
    {
        try {
            throw new \RuntimeException('no');
        }
        catch (\Exception $e) {
        }
    }

    /**
     * One of a multi-catch catches it.
     */
    public function byMultiCatch(): void
    {
        try {
            throw new \RuntimeException('no');
        }
        catch (\TypeError | \RuntimeException $e) {
        }
    }

    /**
     * Caught by something unrelated, so it really does leave.
     */
    public function byNothingRelated(): void
    {
        try {
            throw new \RuntimeException('no');
        }
        catch (\LogicException $e) {
        }
    }

    /**
     * Thrown from the handler, which its own clause does not catch.
     */
    public function fromTheHandler(): void
    {
        try {
            throw new \LogicException('first');
        }
        catch (\LogicException $e) {
            throw new \RuntimeException('second');
        }
    }

    /**
     * Thrown from `finally`, which no clause guards.
     */
    public function fromFinally(): void
    {
        try {
        }
        catch (\RuntimeException $e) {
        }
        finally {
            throw new \RuntimeException('last');
        }
    }

    /**
     * An inner handler catches it; an outer one would have too.
     */
    public function nested(): void
    {
        try {
            try {
                throw new \RuntimeException('no');
            }
            catch (\RuntimeException $e) {
            }
        }
        catch (\Exception $e) {
        }
    }
}
"#,
        &["doc-missing-throws"],
        expect![[r#"
            48:13-48:46 warning [doc-missing-throws] throws \RuntimeException, which the docblock does not declare
            63:13-63:50 warning [doc-missing-throws] throws \RuntimeException, which the docblock does not declare
            77:13-77:48 warning [doc-missing-throws] throws \RuntimeException, which the docblock does not declare
        "#]],
    );
}
