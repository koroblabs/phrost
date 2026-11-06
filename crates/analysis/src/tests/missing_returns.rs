//! A declaration that promises a value from a body control can leave without producing one.
//!
//! PHP raises a `TypeError` on the run that reaches the closing brace, which is what makes this an
//! error rather than a note. Every test here is as much about the silence as about the finding: an
//! abstract method, an interface method, a generator and a `void` all declare something and produce
//! nothing, and none of them is a defect.

use expect_test::expect;

use super::diagnostics::check;

/// A guard with no `else` is reported. An empty body is not, and that is the load-bearing half.
///
/// `function blank(): string {}` is how every extension stub and every hand-kept `stubs/redis.php`
/// is written, and it is also how a declaration somebody has not filled in yet reads. Reporting it
/// says "not every path returns a value" about a body with no paths. Measured: `phrost check` over
/// this repository, whose vendored prelude is 137 such files, went from 5 553 findings to 48.
#[test]
fn a_guard_with_no_else_is_reported_and_an_empty_body_is_not() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function blank(): string
    {
    }

    public function guarded(int $n): string
    {
        if ($n > 0) {
            return 'positive';
        }
    }

    public function covered(int $n): string
    {
        if ($n > 0) {
            return 'positive';
        }
        return 'other';
    }
}
"#,
        expect![[r#"
            8:21-8:28 error [missing-return] Falling off the end returns nothing where string is declared
        "#]],
    );
}

#[test]
fn a_declaration_with_no_body_promises_nothing_this_file_can_break() {
    check(
        r#"
//- /app.php
<?php
interface Finder
{
    public function find(int $id): string;
}

abstract class Base implements Finder
{
    abstract public function build(): string;

    public function find(int $id): string
    {
        return 'found';
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn void_never_and_an_undeclared_return_are_not_promises() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function nothing(): void
    {
    }

    public function stops(): never
    {
    }

    public function untyped()
    {
    }
}
"#,
        expect![[r#""#]],
    );
}

/// A `yield` makes the end legal; the same declared type without one does not.
///
/// The body carries a statement on purpose. Written empty it proved nothing about generators -- an
/// empty body is a stub and is not reported at all -- which is the shape §8 records as a fixture that
/// does not match its own test's name.
#[test]
fn a_generator_finishes_by_running_off_its_end() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function ids(): iterable
    {
        yield 1;
    }

    public function names(): iterable
    {
        $this->ids();
    }
}
"#,
        expect![[r#"
            9:21-9:26 error [missing-return] Falling off the end returns nothing where iterable is declared
        "#]],
    );
}

#[test]
fn a_docblock_return_is_not_something_php_enforces() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    /**
     * @return string
     */
    public function claimed()
    {
    }
}
"#,
        expect![[r#""#]],
    );
}

/// A `?` in the declared type is not permission to return nothing; `null` still has to be written.
///
/// The body carries a statement for the reason the generator fixture above does.
#[test]
fn a_nullable_return_still_has_to_be_returned() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function maybe(): ?string
    {
        $this->maybe();
    }
}
"#,
        expect![[r#"
            4:21-4:26 error [missing-return] Falling off the end returns nothing where ?string is declared
        "#]],
    );
}

#[test]
fn a_body_ending_in_a_call_that_never_returns_produces_nothing_and_needs_no_return() {
    check(
        r#"
//- /src/Assert.php
<?php
class Assert
{
    public static function fail(string $why): never
    {
        throw new \RuntimeException($why);
    }
}

//- /app.php
<?php
class Repo
{
    public function find(int $id): string
    {
        if ($id > 0) {
            return 'found';
        }
        Assert::fail('no such row');
    }

    public function other(int $id): string
    {
        if ($id > 0) {
            return 'found';
        }
        $this->log('no such row');
    }

    public function log(string $why): void
    {
    }
}
"#,
        expect![[r#"
            12:21-12:26 error [missing-return] Falling off the end returns nothing where string is declared
        "#]],
    );
}

#[test]
fn a_free_function_is_judged_the_same_way_as_a_method() {
    check(
        r#"
//- /app.php
<?php
function guarded(int $n): string
{
    if ($n > 0) {
        return 'positive';
    }
}
"#,
        expect![[r#"
            2:10-2:17 error [missing-return] Falling off the end returns nothing where string is declared
        "#]],
    );
}

/// A closure promises what it declares exactly as a function does, and is reported on its keyword.
///
/// PHP's own message for the run that reaches the brace is `{closure}(): Return value must be of
/// type ?int, none returned`. An arrow function is one expression and returns it, so it cannot fall
/// off anything; a closure that yields, one declared `void` and one that returns on every path are
/// the silences beside it.
#[test]
fn a_closure_is_judged_by_what_it_declares_and_reported_on_its_keyword() {
    check(
        r#"
//- /app.php
<?php
function apply(callable $callback): void
{
}

function run(): array
{
    apply(function (int $row): string {
        if ($row > 0) {
            return 'positive';
        }
    });
    return [
        static function (bool $x): ?int {
            if ($x) {
                return 1;
            }
        },
        function (bool $x): int {
            return $x ? 1 : 0;
        },
        function (): void {
        },
        function (): iterable {
            yield 1;
        },
        fn (bool $x): int => $x ? 1 : 0,
    ];
}
"#,
        expect![[r#"
            8:11-8:19 error [missing-return] Falling off the end returns nothing where string is declared
            14:16-14:24 error [missing-return] Falling off the end returns nothing where ?int is declared
        "#]],
    );
}
