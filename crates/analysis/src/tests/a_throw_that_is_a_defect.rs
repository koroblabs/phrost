//! PHP splits its throwables into defects and conditions, and `doc-missing-throws` asks about only
//! the conditions.
//!
//! A condition is one a caller may reasonably handle -- a file that is not there, an id nothing
//! matches -- and documenting it is a promise about the contract. A defect is `ValueError`,
//! `DivisionByZeroError`, `TypeError`, `AssertionError` -- and `InvalidArgumentException`,
//! `DomainException`, `BadMethodCallException`. Nobody writes `@throws \DivisionByZeroError` above
//! a method that divides, because the answer to one is to fix the code rather than to catch it, and
//! neither PhpStorm nor PHPStan asks for it.
//!
//! Two roots, and the language names both. `\Error` is one. `\LogicException` is the other, and
//! PHP's manual says exactly why: "Exception that represents error in the program logic. This kind
//! of exception should lead directly to a fix in your code." `\RuntimeException` is deliberately
//! neither -- "an error which can only be found on runtime" is a condition, and keeping it is what
//! leaves the check worth having.
//!
//! Counted before each half, on the reference project: **81 of 919** were `\ValueError`,
//! `\AssertionError` and `\DivisionByZeroError`, and later **356 of 828** were
//! `\InvalidArgumentException` -- 173 of those from three methods that throw it when a field name
//! does not exist. Not one of either named a contract anybody could act on.
//!
//! By the hierarchy rather than by a list of names: a project's own exception extending `\Error` is
//! making the same statement about itself, and one extending `\Exception` is asking to be
//! documented however it is spelled.

use expect_test::expect;

use super::diagnostics::check_asked_for;

const KIT: &str = r#"
//- /spl.php
<?php
interface Throwable {}
class Exception implements Throwable {}
class RuntimeException extends Exception {}
class LogicException extends Exception {}
class InvalidArgumentException extends LogicException {}
class Error implements Throwable {}
class ValueError extends Error {}
class ArithmeticError extends Error {}
class DivisionByZeroError extends ArithmeticError {}
"#;

#[test]
fn an_error_is_not_asked_for_and_an_exception_is() {
    check_asked_for(
        &format!(
            "{KIT}{}",
            r#"
//- /app.php
<?php
namespace App;

class Repo
{
    /**
     * Divides.
     */
    public function divide(int $by): int
    {
        if ($by === 0) {
            throw new \DivisionByZeroError('zero');
        }

        if ($by < 0) {
            throw new \RuntimeException('negative');
        }

        return 10 / $by;
    }
}
"#
        ),
        &["doc-missing-throws"],
        expect![[r#"
            16:13-16:52 warning [doc-missing-throws] throws \RuntimeException, which the docblock does not declare
        "#]],
    );
}

/// A project's own class extending `\Error` says the same thing about itself.
#[test]
fn a_projects_own_error_is_not_asked_for_either() {
    check_asked_for(
        &format!(
            "{KIT}{}",
            r#"
//- /app.php
<?php
namespace App;

class BrokenState extends \Error {}

class Repo
{
    /**
     * Checks.
     */
    public function check(bool $ok): void
    {
        if (!$ok) {
            throw new BrokenState('broken');
        }
    }
}
"#
        ),
        &["doc-missing-throws"],
        expect![""],
    );
}

/// The second root, and the half that was measured at 356 of 828 on a real project.
#[test]
fn a_logic_exception_is_not_asked_for_and_a_runtime_one_is() {
    check_asked_for(
        &format!(
            "{KIT}{}",
            r#"
//- /app.php
<?php
namespace App;

class Repo
{
    /**
     * Reads a field.
     */
    public function read(string $field, bool $ready): string
    {
        if ($field === '') {
            throw new \InvalidArgumentException('no field named that');
        }

        if (!$ready) {
            throw new \RuntimeException('not loaded yet');
        }

        return $field;
    }
}
"#
        ),
        &["doc-missing-throws"],
        expect![[r#"
            16:13-16:58 warning [doc-missing-throws] throws \RuntimeException, which the docblock does not declare
        "#]],
    );
}

/// And a project's own class extending `\LogicException` says the same thing about itself, while
/// one extending `\RuntimeException` does not.
#[test]
fn a_projects_own_logic_exception_is_not_asked_for_either() {
    check_asked_for(
        &format!(
            "{KIT}{}",
            r#"
//- /app.php
<?php
namespace App;

class MisusedApi extends \LogicException {}
class UpstreamWasDown extends \RuntimeException {}

class Repo
{
    /**
     * Calls out.
     */
    public function call(bool $ok, bool $up): void
    {
        if (!$ok) {
            throw new MisusedApi('wrong order');
        }

        if (!$up) {
            throw new UpstreamWasDown('try later');
        }
    }
}
"#
        ),
        &["doc-missing-throws"],
        expect![[r#"
            19:13-19:51 warning [doc-missing-throws] throws \App\UpstreamWasDown, which the docblock does not declare
        "#]],
    );
}
