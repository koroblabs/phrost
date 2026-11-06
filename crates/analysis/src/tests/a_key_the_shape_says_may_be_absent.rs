//! `$address['city']` where the declaration is `array{city: string, ...}|array{}`.
//!
//! PHPStan reports the *read* and phrost reports the *use*, and for a key that is only ever passed
//! on to something taking `mixed` there is no use to report. Twenty of twenty-eight
//! `offsetAccess.notFound` findings on one real project are exactly that: a key the project's own
//! docblock says may be absent, read straight into an `array<string, mixed>` argument.
//!
//! So this is the read. Only where the declaration *states* the absence, which is two spellings:
//! `array{}` as one arm of the union -- "the whole thing, or nothing" -- and a field written
//! `key?:`. A populated shape that simply does not list a key still says nothing, which is the
//! position `misspelt-key` answers under and which the empty arm must not overturn.
//!
//! What makes it liveable is that everything a body does about an absent key silences it: a
//! truthiness guard removes the empty arm, and `isset`, `array_key_exists` and `??` are read as the
//! presence tests they are. Per *scope*, for the reason `existence_tests` gives -- the guarded read
//! is often nowhere near the test -- but not per file, because a local is a different value in
//! every method and a generated template that coalesces `$address['address']` in twelve of them
//! would otherwise silence the thirteenth.
//!
//! Asked for rather than always on, and the superglobals are out. `$_SERVER` declares nearly every
//! key optional and rightly -- `REQUEST_URI` is absent on the command line -- but that shape is the
//! runtime's documentation of what it may provide, not a claim an author made. Measured before the
//! rule existed: 105 of WordPress's 117 findings were `REQUEST_URI`, `HTTP_HOST` and their
//! neighbours.

use expect_test::expect;

use super::diagnostics::check_asked_for;

const ASKED: &[&str] = &["possibly-absent-key"];

/// Three declarations: the whole-or-nothing union, one with an optional field, and one that is
/// simply complete.
const SHAPES: &str = r#"
//- /src/Shapes.php
<?php

class Shapes
{
    /**
     * @return array{address: string, city: string}|array{}
     */
    public function address(): array
    {
        return [];
    }

    /**
     * @return array{error: bool, data?: string}
     */
    public function result(): array
    {
        return ['error' => TRUE];
    }

    /**
     * @return array{a: string, b: string}
     */
    public function whole(): array
    {
        return ['a' => '', 'b' => ''];
    }
}
"#;

#[test]
fn an_empty_arm_says_every_key_it_lists_elsewhere_may_be_absent() {
    check_asked_for(
        &format!(
            r#"{SHAPES}
//- /src/app.php
<?php

function run(Shapes $shapes): void
{{
    $address = $shapes->address();

    echo $address['address'];
    echo $address['city'];
}}
"#
        ),
        ASKED,
        expect![[r#"
            7:10-7:29 warning [possibly-absent-key] 'address' may not be there; array{} is one of the shapes declared
            8:10-8:26 warning [possibly-absent-key] 'city' may not be there; array{} is one of the shapes declared
        "#]],
    );
}

#[test]
fn a_field_written_with_a_question_mark_says_it_about_itself() {
    // And the required field beside it stays silent, which is what says the rule reads the `?`
    // rather than the shape.
    check_asked_for(
        &format!(
            r#"{SHAPES}
//- /src/app.php
<?php

function run(Shapes $shapes): void
{{
    $result = $shapes->result();

    echo $result['error'];
    echo $result['data'];
}}
"#
        ),
        ASKED,
        expect![[r#"
            8:10-8:25 warning [possibly-absent-key] 'data' may not be there; it is declared optional
        "#]],
    );
}

#[test]
fn a_key_no_arm_declares_is_still_nobodys_business_here() {
    // The position `misspelt-key` answers under, kept: a shape is a claim about the keys it lists
    // and says nothing about the ones it does not, so an empty arm beside it must not turn an
    // undocumented key into a finding.
    check_asked_for(
        &format!(
            r#"{SHAPES}
//- /src/app.php
<?php

function run(Shapes $shapes): void
{{
    $address = $shapes->address();
    $whole = $shapes->whole();

    echo $address['district'];
    echo $whole['c'];
}}
"#
        ),
        ASKED,
        expect![""],
    );
}

#[test]
fn a_truthiness_guard_takes_the_empty_arm_off() {
    check_asked_for(
        &format!(
            r#"{SHAPES}
//- /src/app.php
<?php

function run(Shapes $shapes): void
{{
    $address = $shapes->address();

    if ($address) {{
        echo $address['city'];
    }}
}}
"#
        ),
        ASKED,
        expect![""],
    );
}

#[test]
fn the_three_ways_a_body_tests_for_a_key_all_silence_it() {
    // `isset`, `array_key_exists` and `??`. Each names the key, so each is the author saying it may
    // be absent -- and the read below is the one they wrote the test for.
    check_asked_for(
        &format!(
            r#"{SHAPES}
//- /src/app.php
<?php

function byIsset(Shapes $shapes): void
{{
    $result = $shapes->result();

    if (isset($result['data'])) {{
        echo $result['data'];
    }}
}}

function byArrayKeyExists(Shapes $shapes): void
{{
    $result = $shapes->result();

    if (array_key_exists('data', $result)) {{
        echo $result['data'];
    }}
}}

function byCoalesce(Shapes $shapes): void
{{
    $result = $shapes->result();

    if (($result['data'] ?? NULL) !== NULL) {{
        echo $result['data'];
    }}
}}
"#
        ),
        ASKED,
        expect![[r#"
            16:9-16:25 error [undefined-function] Undefined function array_key_exists()
        "#]],
    );
}

#[test]
fn a_test_in_one_scope_says_nothing_about_another() {
    // Per scope and not per file, which is the whole reason the tests are collected scope by
    // scope: `$result` is a different value in each of these, and a generated template that
    // coalesces one key in twelve methods must not silence the thirteenth.
    check_asked_for(
        &format!(
            r#"{SHAPES}
//- /src/app.php
<?php

function tested(Shapes $shapes): void
{{
    $result = $shapes->result();

    echo $result['data'] ?? '';
}}

function untested(Shapes $shapes): void
{{
    $result = $shapes->result();

    echo $result['data'];
}}
"#
        ),
        ASKED,
        expect![[r#"
            14:10-14:25 warning [possibly-absent-key] 'data' may not be there; it is declared optional
        "#]],
    );
}

#[test]
fn a_superglobal_declares_what_the_runtime_may_provide_and_not_what_an_author_claimed() {
    check_asked_for(SUPERGLOBAL, ASKED, expect![""]);
}

/// The same shape, the same read, one name over -- which is what says the silence above is the rule
/// and not a fixture that declared nothing.
#[test]
fn and_the_same_shape_under_an_ordinary_name_is_reported() {
    check_asked_for(
        &SUPERGLOBAL.replace("$_SERVER", "$server"),
        ASKED,
        expect![[r#"
            10:10-10:32 warning [possibly-absent-key] 'REQUEST_URI' may not be there; it is declared optional
        "#]],
    );
}

/// A shape reached through a name PHP reserves. Written as an assignment because a superglobal is
/// not declared anywhere a fixture can reach, and what the rule reads is the name.
const SUPERGLOBAL: &str = r#"
//- /src/app.php
<?php

/**
 * @param array{REQUEST_URI?: string, HTTP_HOST: string} $data
 */
function run(array $data): void
{
    $_SERVER = $data;

    echo $_SERVER['REQUEST_URI'];
}
"#;
