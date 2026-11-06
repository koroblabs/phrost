//! `@$row['k']` holds what `$row['k']` holds.
//!
//! The `@` silences a *diagnostic of PHP's* -- a warning the engine would print -- and changes no
//! value: a `TypeError` still throws straight through it. Left as an `Unmodelled` it said the
//! expression had no type at all, so `$x = @$a['k'];` was judged by nobody. 622 occurrences in
//! Symfony, 226 in WordPress, 26 in Laravel.
//!
//! So it leaves no node: the operand *is* the expression. The rule that objects to writing `@` at
//! all is a syntax rule and is where that objection belongs; this is only about what the value is.
//!
//! Measured: nothing on the reference project, 2 on Laravel, 27 on Symfony and 105 on WordPress --
//! 72 of those in one vendored ZIP library and the rest on socket and stream handles, which is what
//! code written around `@` looks like. Every one read; `$this->_ftp_data_sock = @socket_create(…);
//! if ($this->_ftp_data_sock < 0)` is a real defect, because PHP 8 hands back a `Socket` object and
//! `< 0` does not test it.

use expect_test::expect;

use super::diagnostics::check;

const ROWS: &str = r#"
//- /src/Rows.php
<?php

class Rows
{
    /**
     * @return array<string, string>|false
     */
    public function all()
    {
        return FALSE;
    }

    public function takes(string $value): string
    {
        return $value;
    }
}
"#;

#[test]
fn a_suppressed_call_is_still_the_value_it_returns() {
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

function run(Rows $rows): string
{{
    $all = @$rows->all();

    return $rows->takes($all);
}}
"#
        ),
        expect![[r#"
            7:25-7:29 warning [possibly-false-argument] $value takes string, and this may be false
        "#]],
    );
}

#[test]
fn and_the_same_call_without_it_says_exactly_the_same_thing() {
    // The control: the `@` is not what produced the finding, and not what would have hidden it.
    check(
        &format!(
            r#"{ROWS}
//- /src/app.php
<?php

function run(Rows $rows): string
{{
    $all = $rows->all();

    return $rows->takes($all);
}}
"#
        ),
        expect![[r#"
            7:25-7:29 warning [possibly-false-argument] $value takes string, and this may be false
        "#]],
    );
}
