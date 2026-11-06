//! `array{...}|array{}`, which is how a project writes "the whole thing, or nothing".
//!
//! Three collapses stood between that docblock and a reader, and each was the same shape: a rule
//! that knew about one spelling and not its neighbour.
//!
//! - **`array{}` interned as a bare `array`**, in lowering and again in the type table. Two
//!   constructors of one collapse, and it threw away the one thing the empty shape says.
//! - **`has_arguments` did not count a union.** A docblock that "says what it contains" beats a
//!   native `: array` hint; a shape counted and a union of shapes did not, so the hint won and
//!   every key went with it. The comment beside the shape arm describes exactly this failure, one
//!   spelling earlier.
//! - **`keyed_of_held` read only `Held::OneOf`.** A union arrives in three spellings and a
//!   `@return` in another file is the interned one -- so the arm that took a key of a union spoke
//!   for the spelling a docblock never produces. `element_of_held` opens with the line that fixes
//!   it, and this now does too.
//!
//! What the empty arm then means is PHP's own rule: reading any key off `[]` is null. And what
//! makes that liveable is the guard -- `array{}` is falsy, so a truthiness test rules it out along
//! with `null` and `false`, which is what the emptiness check above every one of these reads is
//! for.

use expect_test::expect;

use super::diagnostics::check;

/// A method whose docblock says "the whole thing, or nothing", beside a native `array` hint.
const ADDRESS: &str = r#"
//- /src/Address.php
<?php

class Address
{
    /**
     * @return array{city: string, zip: string}|array{}
     */
    public function of(int $id): array
    {
        return [];
    }

    /**
     * @return array{city: string}|array{zip: string}
     */
    public function eitherHalf(int $id): array
    {
        return [];
    }

    public function takes(string $value): string
    {
        return $value;
    }
}
"#;

#[test]
fn a_key_of_a_shape_union_with_an_empty_arm_may_be_absent() {
    check(
        &format!(
            r#"{ADDRESS}
//- /src/app.php
<?php

function run(Address $address, int $id): string
{{
    return $address->takes($address->of($id)['city']);
}}
"#
        ),
        expect![[r#"
            5:28-5:53 warning [possibly-null-argument] $value takes string, and this may be null
        "#]],
    );
}

#[test]
fn an_emptiness_guard_rules_the_empty_arm_out() {
    // The reason the rule above is liveable rather than a noise generator: a project writes the
    // empty arm precisely so it can be tested for, and all three spellings of the test are one
    // truthiness guard underneath. Without this every site the branch made readable was reported.
    check(
        &format!(
            r#"{ADDRESS}
//- /src/app.php
<?php

function run(Address $address, int $id): string
{{
    $row = $address->of($id);

    if ($row === []) {{
        return '';
    }}

    return $address->takes($row['city']);
}}
"#
        ),
        expect![""],
    );
}

/// And the same guard read from the *skipped* side, which is where a body normalises the value.
///
/// `$a = $this->parts(); if (empty($a)) { $a = [...]; } $a['city']` leaves the populated arm on one
/// path and a literal on the other, so the key is there either way. The guard machinery knew that
/// -- `PathRoot::NotNull` takes the empty shape off with the `false` -- and the *skipped* path's
/// half of the same rule did not, which is §8.1's oldest form. Nothing on four corpora moved for
/// it, and four findings of a check that reads shapes did.
#[test]
fn the_path_around_an_emptiness_guard_keeps_the_populated_arm() {
    check(
        &format!(
            r#"{ADDRESS}
//- /src/app.php
<?php

function run(Address $address, int $id): string
{{
    $parts = $address->of($id);

    if (empty($parts)) {{
        $parts = ['city' => 'none', 'zip' => '0'];
    }}

    return $address->takes($parts['city']);
}}
"#
        ),
        expect![""],
    );
}

#[test]
fn a_populated_arm_that_does_not_name_the_key_still_says_nothing() {
    // The position a lone shape already gets, kept: a shape with fields is a claim about the keys
    // it lists and says nothing about the ones it does not, so `misspelt-key` is what speaks there
    // and only for a near miss. An empty shape is the one spelling with nothing left to be
    // incomplete about, which is what separates it from this.
    check(
        &format!(
            r#"{ADDRESS}
//- /src/app.php
<?php

function run(Address $address, int $id): string
{{
    return $address->takes($address->eitherHalf($id)['city']);
}}
"#
        ),
        expect![""],
    );
}

#[test]
fn the_union_beats_the_native_hint_the_way_a_lone_shape_does() {
    // The `has_arguments` half, read through what a wrong argument renders: a bare `array` would
    // print `array` and say nothing about the keys.
    check(
        &format!(
            r#"{ADDRESS}
//- /src/app.php
<?php

function run(Address $address, int $id): string
{{
    return $address->takes($address->of($id));
}}
"#
        ),
        expect![[r#"
            5:28-5:45 error [incompatible-argument] $value takes string, and this is array{city: string, zip: string}|array{}
        "#]],
    );
}
