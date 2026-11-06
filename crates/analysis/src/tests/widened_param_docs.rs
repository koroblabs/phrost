//! A `@param` wider than the native hint says what the callee will take.
//!
//! The direction is the opposite of a return type's, and getting it backwards is how a checker
//! reports code that works. A `@return` *refines*: `array` refined by `list<Item>` tells the caller
//! more precisely what it was handed. A `@param` *widens*: it is the callee stating what it
//! undertakes to handle, and one written beside a narrower hint is how a library carries a legacy
//! argument shape through a deprecation.
//!
//! The rule lives on `phrost_sema::ParamRow::rejects`; this is the behaviour it buys.

use expect_test::expect;

use super::diagnostics::check;

/// The shape this was found on, reduced: Drupal's `StatementInterface::fetchAll` takes `?FetchAs`
/// and documents `FetchAs|int|null`, saying in prose that a deprecated `\PDO::FETCH_*` constant is
/// still accepted. An int there is the declaration working as written.
#[test]
fn an_argument_the_docblock_admits_is_not_reported_against_the_narrower_hint() {
    check(
        r#"
//- /app.php
<?php
enum FetchAs
{
    case Associative;
}

interface Statement
{
    /**
     * @param \FetchAs|int|null $mode
     *   One of the cases of the FetchAs enum, or (deprecated) a \PDO::FETCH_* constant.
     */
    public function fetchAll(?FetchAs $mode = NULL): array;
}

class Legacy
{
    const FETCH_ASSOC = 2;

    public function run(Statement $statement): void
    {
        $statement->fetchAll(self::FETCH_ASSOC);
        $statement->fetchAll(FetchAs::Associative);
    }
}
"#,
        expect![""],
    );
}

/// What the widening must not become: a `@param` naming something else entirely still leaves
/// everything the hint rejects rejected, because the docblock adds an acceptance rather than
/// replacing the list.
#[test]
fn a_value_neither_the_hint_nor_the_docblock_admits_is_still_reported() {
    check(
        r#"
//- /app.php
<?php
interface Statement
{
    /**
     * @param string|int $mode
     */
    public function fetchAll(string $mode): array;
}

class Caller
{
    const SHAPE = ['a'];

    public function run(Statement $statement): void
    {
        $statement->fetchAll(self::SHAPE);
    }
}
"#,
        expect![[r#"
            16:30-16:41 error [incompatible-argument] $mode takes string, and this is array<int, non-falsy-string>
        "#]],
    );
}

/// A *narrower* `@param` changes nothing, and it is the alternatives-not-a-union shape that makes
/// that automatic rather than a special case: the doc type adds an acceptance the hint already
/// covers. Narrowing here would be a finding about a comment -- PHP runs the call, and the runtime
/// checks the hint.
#[test]
fn a_docblock_narrower_than_the_hint_does_not_start_reporting() {
    check(
        r#"
//- /app.php
<?php
declare(strict_types=1);

class Registry
{
    const NAME = 'App\Thing';

    const COUNT = 1;

    /**
     * @param class-string $name
     */
    public function make(string $name): void {}

    public function run(): void
    {
        $this->make(self::NAME);
        $this->make(self::COUNT);
    }
}
"#,
        expect![[r#"
            18:21-18:32 error [incompatible-argument] $name takes class-string, and this is int
        "#]],
    );
}

/// The widening is scoped to what a value *is*, never to whether it may be null. Relaxing that
/// would be `?T where T expected` suppression arriving by the back door, which the project refuses:
/// `possibly-null-argument` reads the declared type directly and is not routed through the rule.
#[test]
fn a_nullable_docblock_beside_a_non_nullable_hint_still_reports_a_null() {
    check(
        r#"
//- /app.php
<?php
class Sink
{
    /**
     * @param string|null $text
     */
    public function write(string $text): void {}

    public function run(?string $maybe): void
    {
        $this->write($maybe);
    }
}
"#,
        expect![[r#"
            11:22-11:28 warning [possibly-null-argument] $text takes string, and this may be null
        "#]],
    );
}
