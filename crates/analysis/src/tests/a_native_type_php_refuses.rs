//! What PHP refuses about a type written in the language, rather than in a comment.
//!
//! Native only, and that is the whole safety of the family: PHP reads no docblocks, so
//! `@return object|mixed` is nothing to it -- and Laravel's facades write exactly that thirty-three
//! times, which is what these rules reported before the docblock half was excluded. A tag declares
//! a member that does not exist, and a member that does not exist cannot fail to compile.
//!
//! Each was run against PHP 8.5.10. Two answers are worth keeping in view: `?int` and `int|null`
//! are both fine, so the `null` an operand list holds is not a duplicate of the `?`; and
//! `self`/`static`/`parent` name a class, so an intersection may hold them while `callable`,
//! `null` and every scalar it may not.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn every_native_type_php_will_not_compile_is_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

interface Reader {}

interface Writer {}

function inUnion(): int|void {}

function underAQuestion(): ?never {}

function asAParameter(void $x): void {}

function twiceOver(): int|INT {}

function bothWays(): true|false {}

function notAClass(): Reader&callable {}

class Holder
{
    public never $impossible;

    public Reader|mixed $loose;
}
"#,
        expect![[r#"
            8:10-8:17 error [refused-declaration] `void` may only be written on its own, and here it is one operand of a type
            10:10-10:24 error [refused-declaration] `never` may only be written on its own, and here it is one operand of a type
            12:28-12:30 error [refused-declaration] `void` may only be a return type, and this is not one
            14:10-14:19 error [refused-declaration] twiceOver writes one type twice in a union
            16:10-16:18 error [refused-declaration] bothWays writes both `true` and `false`, which is what `bool` is
            18:10-18:19 error [refused-declaration] an intersection holds classes, and `callable` is not one
            22:18-22:29 error [refused-declaration] `never` may only be a return type, and this is not one
            24:25-24:31 error [refused-declaration] `mixed` may only be written on its own, and here it is one operand of a type
        "#]],
    );
}

/// And the shapes PHP accepts, three of which look like the rules above. `int|null` is the written
/// form of `?int` and neither is a duplicate; `static|int` is a union of a class and a scalar, which
/// only an *intersection* refuses; and `@method` says nothing PHP reads.
#[test]
fn every_native_type_php_compiles_is_left_alone() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

interface Reader {}

interface Writer {}

/**
 * @method static object|mixed make(string $abstract)
 * @property int|int $counted
 */
class Holder
{
    public ?int $maybe = null;

    public int|null $written = null;

    public Reader&Writer $both;

    public mixed $anything = null;

    public function returns(): void {}

    public function nothing(): never
    {
        exit(1);
    }

    public function relative(): static|int
    {
        return $this;
    }

    public function falsey(): int|false
    {
        return false;
    }
}
"#,
        expect![[r#""#]],
    );
}
