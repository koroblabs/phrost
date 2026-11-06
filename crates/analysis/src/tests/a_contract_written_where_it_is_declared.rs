//! `doc-missing-throws`: a `@throws` on what a declaration overrides documents the declaration.
//!
//! A contract is written where it is declared -- on the interface, or on the abstract method -- and
//! an implementation inherits it. `{@inheritdoc}` was read that way from the start; writing nothing
//! at all was not, so an implementation with its own prose summary had to repeat every `@throws`
//! of every interface it implements or be reported for each.
//!
//! Which is why the reference project, whose implementations write `{@inheritdoc}`, moved by
//! nothing here and Symfony moved by 154 -- the two spellings are a house style, and only one of
//! them was covered.

use expect_test::expect;

use super::diagnostics::check_asked_for;

/// Four implementations: two that inherit a contract, one that adds to it, one that has none.
#[test]
fn a_throws_on_the_interface_covers_the_implementation_that_does_not_repeat_it() {
    check_asked_for(
        r#"
//- /spl.php
<?php
class Exception {}
class RuntimeException extends Exception {}
class DomainFailure extends Exception {}

//- /app.php
<?php
namespace App;

interface Store
{
    /**
     * Fetches one.
     *
     * @throws \RuntimeException
     *   When the store is unreachable.
     */
    public function fetch(int $id): string;
}

abstract class Base
{
    /**
     * @throws \DomainFailure
     */
    abstract public function run(): void;
}

class FileStore implements Store
{
    /**
     * Fetches one from disk, in its own words rather than `{@inheritdoc}`.
     */
    public function fetch(int $id): string
    {
        throw new \RuntimeException('unreachable');
    }
}

class Concrete extends Base
{
    /**
     * Runs it.
     */
    public function run(): void
    {
        throw new \DomainFailure('no');
    }
}

class Loud implements Store
{
    /**
     * Inherits one and throws a second the interface says nothing about.
     */
    public function fetch(int $id): string
    {
        if ($id < 0) {
            throw new \DomainFailure('not inherited');
        }
        throw new \RuntimeException('inherited');
    }
}

class Alone
{
    /**
     * Implements nothing, so there is nothing to inherit.
     */
    public function fetch(): string
    {
        throw new \RuntimeException('its own');
    }
}
"#,
        &["doc-missing-throws"],
        expect![[r#"
            53:13-53:54 warning [doc-missing-throws] throws \DomainFailure, which the docblock does not declare
            66:9-66:47 warning [doc-missing-throws] throws \RuntimeException, which the docblock does not declare
        "#]],
    );
}
