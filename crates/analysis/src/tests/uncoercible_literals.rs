//! A literal PHP cannot convert, where its type says it could.
//!
//! The one place the type judgement's answer is not the whole answer. Without `declare(strict_types=1)`
//! PHP converts a string where an `int` is declared, so `string` really does fit `int` -- and a
//! variable of that type may well hold `'12'`. A **literal** that is not a number cannot: measured on
//! PHP 8.5, `needs_int('bad')`, `$holder->count = 'bad'` and `return 'bad'` from `: int` are each a
//! `TypeError` on every run, and so are `'5 apples'` and `''`. Only `'12'` and `'12.5'` get through.
//!
//! So the rule is asked of the *strict* table against a plain `string`: if a string is rejected
//! there, coercion was the only thing that could have saved this value, and a literal that is not
//! numeric has none. Asking it that way rather than peeling the expectation is what keeps `int|string`
//! and `mixed` silent without a case for either.
//!
//! Every fixture here is deliberately in **weak** mode. In a strict file the ordinary judgement
//! already reports all of it, and a test that declared `strict_types` would pass whether or not any
//! of this existed.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_string_that_is_not_a_number_is_reported_wherever_it_is_written() {
    // The three sites, which is the whole of what has the written expression in hand: an argument,
    // an assignment and a return.
    check(
        r#"
//- /app.php
<?php

function needs_int(int $n): void {}

function needs_float(float $f): void {}

class Holder
{
    public int $count = 0;

    public function get(): int
    {
        return 'nope';
    }

    public function fill(): void
    {
        $this->count = 'bad';
        needs_int('bad');
        needs_float('bad');
        needs_int('5 apples');
        needs_int('');
    }
}
"#,
        expect![[r#"
            13:16-13:22 error [incompatible-return] Cannot return non-falsy-string where int is declared
            18:24-18:29 error [incompatible-assignment] Holder::$count takes int, and this is non-falsy-string
            19:19-19:24 error [incompatible-argument] $n takes int, and this is non-falsy-string
            20:21-20:26 error [incompatible-argument] $f takes float, and this is non-falsy-string
            21:19-21:29 error [incompatible-argument] $n takes int, and this is non-falsy-string
            22:19-22:21 error [incompatible-argument] $n takes int, and this is string
        "#]],
    );
}

#[test]
fn a_string_that_is_a_number_is_what_weak_mode_is_for() {
    // The half that keeps this from being strict mode by the back door. `'12'` converts, `'12.5'`
    // converts with a deprecation PHP itself emits, and leading or trailing whitespace is allowed --
    // all three run, so none of them is a finding.
    check(
        r#"
//- /app.php
<?php

function needs_int(int $n): void {}

class Holder
{
    public int $count = 0;

    public function get(): int
    {
        return '12';
    }

    public function fill(): void
    {
        $this->count = '12';
        needs_int('12');
        needs_int('12.5');
        needs_int(' 12 ');
        needs_int('-3');
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn only_a_literal_says_what_it_holds() {
    // A `string` parameter may hold `'12'`, and the type is all there is to go on -- so the value
    // that is not written out stays silent, which is what it said before this existed.
    check(
        r#"
//- /app.php
<?php

function needs_int(int $n): void {}

class Holder
{
    public function fill(string $unknown, string ...$rest): void
    {
        needs_int($unknown);
        needs_int($rest[0]);
        needs_int('a' . 'b');
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_declaration_that_takes_a_string_takes_this_one() {
    // Asked of the strict table rather than by peeling the expectation, which is what makes these
    // three fall out with no case of their own: a union naming `string` accepts it as a string,
    // `mixed` accepts everything, and an unbound template is not a type to judge against.
    check(
        r#"
//- /app.php
<?php

function takes_either(int|string $v): void {}

function takes_mixed(mixed $v): void {}

/**
 * @template T
 * @param T $v
 */
function takes_template($v): void {}

class Holder
{
    public function fill(): void
    {
        takes_either('bad');
        takes_mixed('bad');
        takes_template('bad');
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_string_whose_text_is_not_its_value_is_left_alone() {
    // This reads the source, so an escape or an interpolation makes the written text and the runtime
    // value two different things. Both are rare where a number is wanted and both are cheaper to
    // decline than to model.
    check(
        r#"
//- /app.php
<?php

function needs_int(int $n): void {}

class Holder
{
    public function fill(string $part): void
    {
        needs_int("1{$part}");
        needs_int("\x31");
    }
}
"#,
        expect![[r#""#]],
    );
}
