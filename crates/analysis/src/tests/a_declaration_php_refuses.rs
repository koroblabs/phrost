//! Declarations PHP will not compile, each visible without leaving the file.
//!
//! The rest of this family is about a pair -- a child and its parent, two traits, two files -- and
//! needs an index. These need nothing, which is why they are easy to leave out and why nothing was
//! reporting them: each is written once and never thought about again. The alternative to a red
//! squiggle is a white screen.
//!
//! Every rule was run against PHP 8.5.10, and two of them are narrower than they look: a
//! trait's `abstract private` is **legal** -- the using class fulfils it with a private method of
//! its own, and Symfony writes that three times -- and an interface's method is abstract whether the
//! word is written or not, so a body on one is the same fatal.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn what_php_will_not_compile_is_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Readonlys
{
    public readonly $untyped;

    public readonly int $defaulted = 1;

    public static readonly int $shared;
}

abstract class Abstracts
{
    abstract public function withBody(): void {}

    abstract private function hidden(): void;
}

interface Contract
{
    public function withBody(): void {}
}

enum Holder
{
    case A;

    public int $n = 1;
}

enum Plain
{
    case Valued = 'v';
}

enum Backed: string
{
    case Bare;
}

abstract class Cancelled
{
    abstract final public function both(): void;

    const CLASS = 'taken';
}

interface Sealed
{
    final public function nothing(): void;
}
"#,
        expect![[r#"
            6:21-6:29 error [refused-declaration] readonly $untyped has no type, and PHP requires one
            8:25-8:35 error [refused-declaration] readonly $defaulted cannot have a default value
            10:32-10:39 error [refused-declaration] static $shared cannot be readonly
            15:30-15:38 error [refused-declaration] abstract withBody() cannot have a body
            17:31-17:37 error [refused-declaration] abstract hidden() cannot be private -- nothing could implement it
            22:21-22:29 error [refused-declaration] abstract withBody() cannot have a body
            29:16-29:18 error [refused-declaration] an enum cannot hold a property, and $n is one
            34:10-34:16 error [refused-declaration] Valued is a case of an enum with no backing type, so it cannot have a value
            39:10-39:14 error [refused-declaration] Bare is a case of a backed enum, so it must have a value
            44:36-44:40 error [refused-declaration] both() is abstract, so it cannot also be final
            46:11-46:16 error [refused-declaration] `CLASS` is the name-fetching syntax, so no constant may be called it
            51:27-51:34 error [refused-declaration] nothing() is abstract, so it cannot also be final
        "#]],
    );
}

/// What is legal and must stay silent, including the two narrower rules.
#[test]
fn what_php_compiles_is_left_alone() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

trait Requires
{
    abstract private function hidden(): void;
}

abstract class Fine
{
    public readonly int $typed;

    public static int $shared = 0;

    abstract public function later(): void;

    public function withBody(): void {}
}

interface Contract
{
    public function later(): void;
}

enum Holder
{
    case A;

    const LABEL = 'a';

    public function label(): string
    {
        return self::LABEL;
    }
}

enum Suit: string
{
    case Hearts = 'h';
}

final class Closed
{
    final public function fixed(): void {}
}
"#,
        expect![""],
    );
}
