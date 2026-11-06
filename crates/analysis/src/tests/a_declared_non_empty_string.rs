//! `non-empty-string` and `non-falsy-string`, judged where they are declared.
//!
//! Both folded into `string` at lowering, so a parameter, a return or a property declaring one took a
//! `''` without a word. Read the way PHPStan reads them at level 7: a plain `string` is reported
//! where one that holds something is declared, because the empty one is a value the declaration
//! refuses -- and a value that does hold something, by the literal that wrote it, the `.` that built
//! it or the guard it passed, is not.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_string_that_may_be_empty_is_reported_where_one_that_holds_something_is_declared() {
    check(
        r#"
//- /src/Names.php
<?php
namespace App;

/** @param non-empty-string $name */
function named(string $name): void {}

/** @param non-falsy-string $name */
function truthy(string $name): void {}

final class Holder
{
    /** @var non-empty-string */
    public string $name = 'x';
}

/** @return non-empty-string */
function returned(string $s): string
{
    return $s;
}

function run(string $s, Holder $holder): void
{
    named($s);
    named('');
    truthy('0');
    named($s . $s);
    $holder->name = $s;
}
"#,
        expect![[r#"
            19:12-19:14 error [incompatible-return] Cannot return string where non-empty-string is declared
            24:11-24:13 error [incompatible-argument] $name takes non-empty-string, and this is string
            25:11-25:13 error [incompatible-argument] $name takes non-empty-string, and this is string
            26:12-26:15 error [incompatible-argument] $name takes non-falsy-string, and this is non-empty-string
            27:11-27:18 error [incompatible-argument] $name takes non-empty-string, and this is string
            28:21-28:23 error [incompatible-assignment] App\Holder::$name takes non-empty-string, and this is string
        "#]],
    );
}

/// Every value here holds something by what wrote it or by the guard it passed. The last call is
/// the control: a plain `string` past every guard, so an empty expectation would mean the check
/// had not run.
#[test]
fn a_string_known_to_hold_something_is_not_reported() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function strlen(string $string): int {}
/** @return non-falsy-string */
function md5(string $string, bool $binary = false): string {}

//- /src/Holds.php
<?php
namespace App;

/** @param non-empty-string $name */
function named(string $name): void {}

/**
 * @assert-if-true non-empty-string $s
 */
function isNonEmpty(string $s): bool
{
    return $s !== '';
}

function run(string $s, ?string $n, int $i): void
{
    named('0');
    named($s . 'x');
    named("a{$s}");
    named((string) $i);
    named(Holds::class);
    named(md5($s));
    /** @var non-empty-string $annotated */
    $annotated = $s;
    named($annotated);
    if ($s !== '') {
        named($s);
    }
    if (strlen($s) > 0) {
        named($s);
    }
    if ($s) {
        named($s);
    }
    if (!empty($s)) {
        named($s);
    }
    if ($n !== null && $n !== '') {
        named($n);
    }
    if (isNonEmpty($s)) {
        named($s);
    }
    named($s);
}

final class Holds {}
"#,
        expect![[r#"
            44:11-44:13 error [incompatible-argument] $name takes non-empty-string, and this is string
        "#]],
    );
}
