//! A constant whose value names other constants: `define('HOUR_IN_SECONDS', 60 * MINUTE_IN_SECONDS)`.
//!
//! The declaring file cannot say what `MINUTE_IN_SECONDS` is, so its parts are kept and the type
//! is settled where it is asked, against every file -- by PHP's own rules for the operators joining
//! them, and by PHP's own order for which constant a name reaches.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_value_built_from_other_constants_has_the_type_they_make() {
    check(
        r#"
//- /src/defs.php
<?php
define('MINUTE_IN', 60);
define('HOUR_IN', 60 * MINUTE_IN);
define('DAY_IN', 24 * HOUR_IN);
define('READ', 1);
define('WRITE', 2);
define('BOTH', READ | WRITE);
define('LOOP', LOOP);
define('LIMIT', 5);

//- /src/Use.php
<?php
namespace App;

const SHADOW = UNKNOWABLE;
const LIMIT = UNKNOWABLE;
const VIA = LIMIT;

class Use
{
    public function run(): void
    {
        $day = DAY_IN;
        $half = HOUR_IN / 2;
        $both = BOTH;
        $loop = LOOP;
        $shadow = SHADOW;
        $via = VIA;
        $d@day@ay;
        $h@half@alf;
        $b@both@oth;
        $l@loop@oop;
        $s@shadow@hadow;
        $v@via@ia;
    }
}
"#,
        &[
            Query::hover("three constants deep", "day"),
            Query::hover("a division", "half"),
            Query::hover("flags joined", "both"),
            Query::hover("a constant naming itself", "loop"),
            Query::hover("a constant naming one nothing declares", "shadow"),
            Query::hover(
                "a name reaching the namespace's constant, not the global",
                "via",
            ),
        ],
        expect![[r#"
            three constants deep
            - code: int $day
            - note: assigned
            a division
            - code: int|float $half
            - note: assigned
            flags joined
            - code: int $both
            - note: assigned
            a constant naming itself
            - code: $loop
            - note: assigned
            - note: no declared type
            a constant naming one nothing declares
            - code: $shadow
            - note: assigned
            - note: no declared type
            a name reaching the namespace's constant, not the global
            - code: $via
            - note: assigned
            - note: no declared type
        "#]],
    );
}
