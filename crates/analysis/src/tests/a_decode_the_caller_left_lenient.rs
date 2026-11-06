//! A failure branch the call's own flag argument rules out.
//!
//! `base64_decode()` is declared `string|false`, and the `false` is what it answers for input
//! outside the alphabet -- **only with `$strict` set**. Left out, which is how nearly every call
//! writes it, invalid characters are silently discarded and a string always comes back. So the
//! declaration is true of the two-argument form and a report against the one-argument form is a run
//! that cannot happen.
//!
//! The claim is a conditional return type in the vendored set rather than a rule here, because the
//! docblock language can say it exactly: see `the_added_type_tags_survive` for the tripwire that
//! keeps it there. What this pins is the reading -- and specifically the arm where the *absent*
//! argument is settled by the declaration's own default, which is the one a caller relies on.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_flag_left_out_takes_the_branch_its_default_names_and_one_written_takes_the_other() {
    check(
        r#"
//- /src/stub.php
<?php

/**
 * @return ($strict is true ? string|false : string)
 */
function base64_decode(string $string, bool $strict = false): string|false {}

//- /src/app.php
<?php

class Inbox
{
    public function run(string $raw): void
    {
        $lenient = base64_decode($raw);
        $written = base64_decode($raw, FALSE);
        $strict = base64_decode($raw, TRUE);
        $len@lenient@ient;
        $wri@written@tten;
        $str@strict@ict;
    }
}
"#,
        &[
            Query::hover("the flag left out", "lenient"),
            Query::hover("the default written at the call", "written"),
            Query::hover("the strict form, which really can fail", "strict"),
        ],
        expect![[r#"
            the flag left out
            - code: string $lenient
            - note: assigned
            the default written at the call
            - code: string $written
            - note: assigned
            the strict form, which really can fail
            - code: string|false $strict
            - note: assigned
        "#]],
    );
}
