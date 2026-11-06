//! `doc-missing-throws`: a `throw new X` under a docblock with no `@throws` that covers it.
//!
//! Its own rule rather than part of `doc-missing-tags`, so a project that documents its parameters
//! and not its exceptions -- which is what Drupal's standard asks for -- can turn this on alone. The
//! report is on the `throw`, where the exception is written and where the quick fix is offered.
//! Covered means the class or anything it extends: `@throws \Exception` above a `RuntimeException`
//! is how a docblock summarises, and a rethrow of a caught variable is not read at all.

use expect_test::expect;

use super::diagnostics::check_asked_for;

#[test]
fn a_throw_no_throws_tag_covers_is_reported_where_it_is_written() {
    check_asked_for(
        r#"
//- /spl.php
<?php
class Exception {}
class RuntimeException extends Exception {}
class InvalidArgumentException extends Exception {}

//- /app.php
<?php
namespace App;

class Missing extends \RuntimeException {}

class Repo
{
    /**
     * Finds one.
     *
     * @throws \InvalidArgumentException
     *   When the id is negative.
     */
    public function find(int $id): int
    {
        if ($id < 0) {
            throw new \InvalidArgumentException('negative');
        }
        if ($id === 0) {
            throw new Missing('none');
        }
        return $id;
    }

    /**
     * Summarised with the base class, which covers the leaf.
     *
     * @throws \Exception
     *   When it cannot.
     */
    public function summarised(int $id): int
    {
        if ($id < 0) {
            throw new Missing('none');
        }
        return $id;
    }

    /**
     * Rethrows what it caught, which is not read, and throws from a closure, which is the
     * closure's.
     */
    public function passes(callable $work): void
    {
        try {
            $work();
        }
        catch (\Exception $e) {
            throw $e;
        }
        $fail = function (): void {
            throw new Missing('inside');
        };
        $fail();
    }

    public function undocumented(): void
    {
        throw new Missing('no docblock at all');
    }
}
"#,
        &["doc-missing-throws"],
        expect![[r#"
            20:13-20:38 warning [doc-missing-throws] throws \App\Missing, which the docblock does not declare
        "#]],
    );
}
