//! Which returned values are reported, and -- the half that matters -- which are not.
//!
//! A wrong "declares Foo, returns Bar" on somebody's framework code is worse than saying nothing,
//! so most of what is asserted here is silence: an open class, a generator, a closure's own
//! `return`, a value the index could not type.
//!
//! Every case runs through `with_open_scopes`, so each also asserts the answer does not depend on
//! which files are open.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;

use crate::IndexView;

/// Every finding in `/main.php`, whole -- range, declared type, returned type.
///
/// The whole result and not a substring: a check that reports one right thing and four wrong ones
/// has to fail here, which a "contains" assertion would let through.
fn check(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let file = world.file_id("main.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let found = view.incompatible_returns();
        if found.is_empty() {
            return "(nothing)\n".to_owned();
        }
        let mut out = String::new();
        for one in found {
            let _ = writeln!(
                out,
                "{}  declares {}, returns {}",
                world.render_range(file, one.range),
                one.declared,
                one.returned
            );
        }
        out
    });

    expect.assert_eq(&rendered);
}

#[test]
fn a_string_literal_cannot_be_an_int_under_strict_types() {
    // `int` widened to `float` is the one scalar widening the directive keeps, so it stays silent
    // in the same file that reports the string.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);
function f(): int {
    return 'x';
}
function g(): float {
    return 1;
}
"#,
        expect![[r#"
            4:12-4:15  declares int, returns non-falsy-string
        "#]],
    );
}

#[test]
fn weak_mode_still_cannot_convert_a_string_that_is_not_a_number() {
    // The same body, without the directive. This used to be silent, and the reason written here was
    // that "which of those a string is is a fact about data nothing here has" -- which is true of a
    // `string`-typed value and false of the one written on the line: `'x'` is a `TypeError` on every
    // run, weak mode or not. The type judgement is still right to accept a `string`; what settles it
    // is the literal, and `fit_written_value` is where the two meet.
    check(
        r#"
//- /main.php
<?php
function f(): int {
    return 'x';
}
"#,
        expect![[r#"
            3:12-3:15  declares int, returns non-falsy-string
        "#]],
    );
}

#[test]
fn weak_mode_converts_a_string_that_is_a_number() {
    // The half the note above was right about, and the half that keeps the rule from being strict
    // mode by the back door: `'7'` converts, so the line runs and there is nothing to report.
    check(
        r#"
//- /main.php
<?php
function f(): int {
    return '7';
}
function g(): int {
    return sprintf('%d', 1);
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn a_method_is_judged_against_its_own_declaration() {
    check(
        r#"
//- /main.php
<?php
class Invoice {}
class Payment {}

class Repo {
    public function find(): Payment {
        return new Invoice();
    }
}
"#,
        expect![[r#"
            7:16-7:29  declares \Payment, returns Invoice
        "#]],
    );
}

#[test]
fn a_return_inside_a_closure_belongs_to_the_closure() {
    // The classic way this check goes wrong. The `int` on the closure and the `string` on the
    // function around it must not be crossed -- and neither body is wrong, so both are silent.
    check(
        r#"
//- /main.php
<?php
function outer(): string {
    $inner = function (): int {
        return 7;
    };
    return 'ok';
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn a_closure_is_judged_against_its_own_declaration() {
    // This was a stated gap for as long as the check existed: a closure's declared return is a
    // body-local `TypeRefId` that nothing interns, and the judgement's expected side had to be
    // interned. `Expectation` carries that difference already -- it was written for a `@var` that
    // had the same shape -- so the closure, the arrow function and the static closure are judged
    // like any method now. Found by asking PHPStan for the sites of one rule: a `preg_replace`
    // inside a `function (string $v): string` was reported by it and by nothing here.
    check(
        r#"
//- /main.php
<?php
function outer(): string {
    $inner = function (): int {
        return 'x';
    };
    $arrow = fn (): int => 'x';
    $static = static function (): int {
        return 'x';
    };
    return 'ok';
}
"#,
        expect![[r#"
            4:16-4:19  declares int, returns non-falsy-string
            6:28-6:31  declares int, returns non-falsy-string
            8:16-8:19  declares int, returns non-falsy-string
        "#]],
    );
}

#[test]
fn a_closure_cannot_launder_the_enclosing_declaration() {
    // The other half of the scope rule: the function's own `return` is still judged even when a
    // closure is written above it.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);
function outer(): int {
    $inner = function (): string {
        return 'fine';
    };
    return 'x';
}
"#,
        expect![[r#"
            7:12-7:15  declares int, returns non-falsy-string
        "#]],
    );
}

#[test]
fn a_generator_is_skipped_whole() {
    // A `yield` makes the declared type describe the generator, so the `return` inside it hands a
    // value to `getReturn()` and has nothing to do with `iterable`.
    check(
        r#"
//- /main.php
<?php
function ids(): iterable {
    yield 1;
    return 'done';
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn a_closure_that_yields_does_not_excuse_the_function_around_it() {
    // The flag is per body, and this is what that buys: the outer function is not a generator, so
    // its own wrong `return` is still reported.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);
function outer(): int {
    $gen = function () {
        yield 1;
    };
    return 'x';
}
"#,
        expect![[r#"
            7:12-7:15  declares int, returns non-falsy-string
        "#]],
    );
}

#[test]
fn every_branch_of_a_body_is_walked() {
    // `if`, `switch`, `try`, `foreach` -- a `return` in any of them is the function's.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);
function f(int $n, array $rows): int {
    if ($n > 0) {
        return 'a';
    }
    switch ($n) {
        case 1:
            return 'b';
    }
    try {
        return 'c';
    } catch (\Throwable $e) {
        return 'd';
    } finally {
    }
    foreach ($rows as $row) {
        return 'e';
    }
    while ($n) {
        return 'f';
    }
    return 0;
}
"#,
        expect![[r#"
            5:16-5:19  declares int, returns non-falsy-string
            9:20-9:23  declares int, returns non-falsy-string
            12:16-12:19  declares int, returns non-falsy-string
            14:16-14:19  declares int, returns non-falsy-string
            18:16-18:19  declares int, returns non-falsy-string
            21:16-21:19  declares int, returns non-falsy-string
        "#]],
    );
}

#[test]
fn a_bare_return_is_never_a_finding() {
    check(
        r#"
//- /main.php
<?php
function f(): void {
    return;
}
class Box {
    public function reset(): void {
        return;
    }
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn a_value_returned_from_a_void_declaration_belongs_to_another_check() {
    // Wrong code, and a fatal, but the mistake is the declaration rather than the value's type --
    // "declares void, returns int" names the wrong half of it.
    check(
        r#"
//- /main.php
<?php
function f(): void {
    return 1;
}
function g(): never {
    return 1;
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn an_open_class_where_an_unrelated_interface_is_declared_is_refused() {
    // A subclass of `Loose` might implement `Runs`, which used to keep this silent. Nothing declared
    // makes a `Loose` one -- and `new Loose()` is exactly a `Loose` -- so open or final, it is refused.
    check(
        r#"
//- /main.php
<?php
interface Runs {}
class Loose {}
final class Sealed {}

function a(): Runs {
    return new Loose();
}
function b(): Runs {
    return new Sealed();
}
"#,
        expect![[r#"
            7:12-7:23  declares \Runs, returns Loose
            10:12-10:24  declares \Runs, returns Sealed
        "#]],
    );
}

#[test]
fn a_subclass_satisfies_its_ancestor() {
    check(
        r#"
//- /main.php
<?php
class Base {}
class Sub extends Base {}

function f(): Base {
    return new Sub();
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn a_relative_return_type_is_never_judged() {
    // `self` and `static` mean a different class per receiver, and the judgement leaves both
    // undecided rather than guessing which.
    check(
        r#"
//- /main.php
<?php
class Row {
    public function self_(): self {
        return new Row();
    }
    public function static_(): static {
        return $this;
    }
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn a_possible_null_is_left_to_the_null_vocabulary() {
    // Wrong code, and reported by nothing today. Under a type code it would read as a claim about
    // `?Row` being the wrong class, which is not the mistake.
    check(
        r#"
//- /main.php
<?php
class Row {}

function find(): ?Row {
    return null;
}
function f(): Row {
    return find();
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn a_value_the_index_cannot_type_says_nothing() {
    check(
        r#"
//- /main.php
<?php
function f(): int {
    return \Vendor\Missing::make();
}
function g(int $n): int {
    return $n;
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn a_docblock_return_is_judged_like_a_written_one() {
    // `@return` is what most older code says instead of a signature, and the two are ranked rather
    // than merged everywhere else in this index.
    check(
        r#"
//- /main.php
<?php
class Row {}
class Other {}

/**
 * @return Row
 */
function f() {
    return new Other();
}
"#,
        expect![[r#"
            9:12-9:23  declares \Row, returns Other
        "#]],
    );
}

#[test]
fn a_union_is_reported_only_when_every_branch_is_impossible() {
    // `string|array` has nowhere to land, so no narrowing this layer is missing could rescue it.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);

function source(): string|array {
    return 'x';
}

function f(): int {
    return source();
}
"#,
        expect![[r#"
            9:12-9:20  declares int, returns string|array
        "#]],
    );
}

#[test]
fn a_union_with_one_branch_that_fits_is_left_alone() {
    // The rule `alternatives` states, in the smallest shape that shows it: `int|false` where an
    // `int` is declared is rejected by the judgement -- one branch cannot land -- and stays quiet
    // here because the other one can.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);

function maybe(): int|false {
    return 1;
}

function f(): int {
    return maybe();
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn a_parameters_own_union_arriving_unnarrowed_is_judged_branch_by_branch() {
    // Neither family that rule shelters can produce this: no call decided the union, and no guard
    // stands between the declaration and the `return`. A guard that rules the wrong branch out, and
    // a union that arrives through a call, both keep the rule.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);

final class A {}
final class B {}

function either(): A|B { return new B(); }

function unguarded(A|B $x): B {
    return $x;
}

function guarded(A|B $x): B {
    if ($x instanceof A) {
        throw new LogicException();
    }
    return $x;
}

function called(): B {
    return either();
}
"#,
        expect![[r#"
            10:12-10:14  declares \B, returns A|B
        "#]],
    );
}

#[test]
fn a_bool_is_two_branches_to_that_rule() {
    // A `bool` against one of its halves is rejected by the judgement, as an argument -- and here
    // it is a union with a branch that fits, the same family as `int|false` above. A value no half
    // of which fits is still reported.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);

class Failure {}

function check(): bool { return true; }

function done(): true|Failure {
    return check();
}

function count_of(): int {
    return check();
}
"#,
        expect![[r#"
            13:12-13:19  declares int, returns bool
        "#]],
    );
}

#[test]
fn a_guard_that_settles_falsehood_leaves_only_the_branch_that_fits() {
    // The same union, guarded. Nothing here has to decide anything: falsy narrowing has already
    // left an `int` at the `return`, so the judgement accepts it outright and the rule above is
    // never reached.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);

function maybe(): int|false {
    return 1;
}

function f(): int {
    $value = maybe();
    if ($value !== false) {
        return $value;
    }
    return 0;
}
"#,
        expect![[r#"
            (nothing)
        "#]],
    );
}

#[test]
fn a_stub_conditional_return_narrows_the_union_its_signature_declares() {
    // `str_replace` as `third_party/php-prelude/standard.php` declares it, verbatim. The signature
    // says `array|string` and the conditional says which of the two this call gets, so what is
    // asserted is the *rendered* type: a declaration the reader cannot settle would report the
    // whole union here, and one it can reports the branch the third argument picked.
    //
    // Declared `int` on purpose. A `string` return would be accepted and prove nothing about which
    // of the two branches was read.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);

/**
 * @param string|array<string> $search
 * @param string|array<string> $replace
 * @param string|array<string> $subject
 *
 * @return ($subject is string ? string : array<string>)
 *
 * @pure
 */
function str_replace(
    array|string $search,
    array|string $replace,
    array|string $subject,
    ?int &$count = null,
): array|string {}

function f(string $text): int {
    return str_replace('a', 'b', $text);
}

function g(array $lines): int {
    return str_replace('a', 'b', $lines);
}
"#,
        expect![[r#"
            21:12-21:40  declares int, returns string
            25:12-25:41  declares int, returns string[]
        "#]],
    );
}

#[test]
fn a_branch_that_is_itself_a_conditional_keeps_the_contract() {
    // What `str_replace` said before, and what this pinned as a loss: the else branch is a second
    // conditional, which had no type to lower out of it, so the whole contract was dropped and the
    // declaration answered with its own `array|string`. A nested conditional is the union of its
    // branches now -- whichever way its own test goes it is one of them -- and the outer test, which
    // the call settles, picks `string` for a string subject. Laravel's `app()` and Mockery's
    // `allows()` are written the same way.
    check(
        r#"
//- /main.php
<?php
declare(strict_types=1);

/**
 * @param string|array<string> $search
 * @param string|array<string> $replace
 * @param string|array<string> $subject
 *
 * @return ($subject is string ? string : (
 *   $subject is array<string> ? array<string> : string|array<string>
 * ))
 *
 * @pure
 */
function str_replace(
    array|string $search,
    array|string $replace,
    array|string $subject,
    ?int &$count = null,
): array|string {}

function f(string $text): int {
    return str_replace('a', 'b', $text);
}
"#,
        expect![[r#"
            23:12-23:40  declares int, returns string
        "#]],
    );
}
