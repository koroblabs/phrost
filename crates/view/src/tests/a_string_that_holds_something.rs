//! What a string is known to hold, as hover reads it: `non-empty-string` and `non-falsy-string` are
//! types now, and a value says it is one by what built it or by the guard it passed.
//!
//! The built half -- a quoted literal by its text, a `.` by the piece that holds the most, a cast by
//! what its operand spells -- and the guarded half -- `!== ''`, `strlen() > 0`, a truthiness test,
//! `?:` -- are tested here together, because each is one claim about the same three types and a
//! hover over the value is where a person reads it.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_string_built_in_place_holds_what_its_pieces_do() {
    check_view(
        r#"
//- /stubs/builtins.php  stub
<?php
function sprintf(string $format, mixed ...$values): string {}
/** @return class-string */
function get_class(object $object): string {}

//- /src/Build.php
<?php
namespace App;

class Build
{
    /** @param non-empty-string $prefix */
    public function go(string $s, string $prefix, int $i, float $f): void
    {
        $empty = '';
        $zero = '0';
        $word = 'word';
        $escaped = "\060";
        $joined = $s . $s;
        $suffixed = $s . '_id';
        $prefixed = $prefix . $s;
        $counted = $s . $i;
        $held = "{$prefix}";
        $cast = (string) $i;
        $floated = (string) $f;
        $plain = (string) $s;
        $named = self::class;
        $classed = get_class($this);
        $formatted = sprintf('id-%d', $i);
        $bare = sprintf('%s', $s);
        $padded = sprintf('%02s', $s);
        $appended = $s;
        $appended .= 'x';
        $e@empty@mpty; $z@zero@ero; $w@word@ord; $e@escaped@scaped; $j@joined@oined;
        $s@suffixed@uffixed; $p@prefixed@refixed; $c@counted@ounted; $h@held@eld; $c@cast@ast;
        $f@floated@loated; $p@plain@lain; $n@named@amed; $c@classed@lassed;
        $f@formatted@ormatted; $b@bare@are; $p@padded@added; $a@appended@ppended;
    }
}
"#,
        &[
            Query::variable_type("an empty literal", "empty"),
            Query::variable_type("'0'", "zero"),
            Query::variable_type("a word", "word"),
            Query::variable_type("an escape that spells '0'", "escaped"),
            Query::variable_type("two strings that may be empty", "joined"),
            Query::variable_type("a literal tail", "suffixed"),
            Query::variable_type("a non-empty head", "prefixed"),
            Query::variable_type("an int's digits", "counted"),
            Query::variable_type("an interpolated non-empty string", "held"),
            Query::variable_type("a cast int", "cast"),
            Query::variable_type("a cast float", "floated"),
            Query::variable_type("a cast string", "plain"),
            Query::variable_type("a class name", "named"),
            Query::variable_type("get_class()", "classed"),
            Query::variable_type("a format with text", "formatted"),
            Query::variable_type("a format of nothing but a conversion", "bare"),
            Query::variable_type("a padded conversion", "padded"),
            Query::variable_type("a literal appended", "appended"),
        ],
        expect![[r#"
            an empty literal
            - string
            '0'
            - non-empty-string
            a word
            - non-falsy-string
            an escape that spells '0'
            - non-empty-string
            two strings that may be empty
            - string
            a literal tail
            - non-falsy-string
            a non-empty head
            - non-empty-string
            an int's digits
            - non-empty-string
            an interpolated non-empty string
            - non-empty-string
            a cast int
            - non-empty-string
            a cast float
            - non-empty-string
            a cast string
            - string
            a class name
            - class-string<Build>
            get_class()
            - class-string
            a format with text
            - non-falsy-string
            a format of nothing but a conversion
            - string
            a padded conversion
            - non-falsy-string
            a literal appended
            - non-falsy-string
        "#]],
    );
}

#[test]
fn a_guard_that_rules_out_the_empty_string_narrows_what_a_string_holds() {
    check_view(
        r#"
//- /stubs/builtins.php  stub
<?php
function strlen(string $string): int {}
function str_starts_with(string $haystack, string $needle): bool {}
function in_array(mixed $needle, array $haystack, bool $strict = false): bool {}
function is_numeric(mixed $value): bool {}
/**
 * @template T
 * @param array<T> $array
 * @return array<T>
 */
function array_filter(array $array): array {}

//- /src/Guards.php
<?php
namespace App;

class Guards
{
    /**
     * @assert-if-true non-empty-string $s
     */
    public static function isNonEmpty(string $s): bool { return $s !== ''; }

    /** @param list<string> $names */
    public function go(string $s, ?string $n, mixed $m, array $names): void
    {
        if ($s !== '') { $@strict@s; }
        if ($n !== '') { $@nullable@n; }
        if ($m !== '') { $@mixed@m; }
        if (strlen($s) > 0) { $@length@s; }
        if ($s) { $@truthy@s; }
        if (!empty($n)) { $@empty@n; }
        if (str_starts_with($s, '$')) { $@starts@s; }
        if (in_array($s, ['draft', 'sent'], true)) { $@strictly@s; }
        if (in_array($s, ['draft', '0'])) { $@loosely@s; }
        if (is_numeric($s)) { $@numeric@s; }
        if (self::isNonEmpty($s)) { $@asserted@s; }
        $or = $s ?: 'none';
        $o@elvis@r;
        $kept = array_filter($names);
        $k@filtered@ept;
    }
}
"#,
        &[
            Query::variable_type("!== ''", "strict"),
            Query::variable_type("!== '' keeps the null", "nullable"),
            Query::variable_type("!== '' says nothing of a mixed", "mixed"),
            Query::variable_type("strlen() > 0", "length"),
            Query::variable_type("truthy", "truthy"),
            Query::variable_type("!empty()", "empty"),
            Query::variable_type("a needle found", "starts"),
            Query::variable_type("a strict search", "strictly"),
            Query::variable_type("a loose search among a number", "loosely"),
            Query::variable_type("is_numeric()", "numeric"),
            Query::variable_type("an @assert-if-true", "asserted"),
            Query::variable_type("?:", "elvis"),
            Query::variable_type("array_filter()", "filtered"),
        ],
        expect![[r#"
            !== ''
            - non-empty-string
            !== '' keeps the null
            - ?non-empty-string
            !== '' says nothing of a mixed
            - mixed
            strlen() > 0
            - non-empty-string
            truthy
            - non-falsy-string
            !empty()
            - non-falsy-string
            a needle found
            - non-falsy-string
            a strict search
            - non-falsy-string
            a loose search among a number
            - non-empty-string
            is_numeric()
            - non-empty-string
            an @assert-if-true
            - non-empty-string
            ?:
            - non-falsy-string
            array_filter()
            - non-falsy-string[]
        "#]],
    );
}

#[test]
fn a_join_keeps_what_every_branch_holds() {
    check_view(
        r#"
//- /src/Joins.php
<?php
namespace App;

class Joins
{
    public function go(bool $c, string $s): void
    {
        $either = '';
        if ($c) { $either = 'x'; }
        $both = 'y';
        if ($c) { $both = 'x'; }
        $normalised = $s;
        if ($normalised === '') { $normalised = 'default'; }
        $e@either@ither; $b@both@oth; $n@normalised@ormalised;
    }
}
"#,
        &[
            Query::variable_type("'' or 'x'", "either"),
            Query::variable_type("'y' or 'x'", "both"),
            Query::variable_type("an empty string replaced", "normalised"),
        ],
        expect![[r#"
            '' or 'x'
            - string
            'y' or 'x'
            - non-falsy-string
            an empty string replaced
            - non-empty-string
        "#]],
    );
}
