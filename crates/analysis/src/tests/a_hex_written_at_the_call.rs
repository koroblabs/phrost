//! `hex2bin()` with the hex written at the call.
//!
//! The end of the recogniser in `phrost_view::literal_returns`, asserted where it matters: whether
//! a checker reports the failure branch. `hex2bin` answers `false` for an odd length or a character
//! outside its alphabet and for nothing else, so a literal settles it exactly -- and a value read
//! from data still cannot be vouched for.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn hex_written_at_the_call_cannot_fail_and_a_value_read_from_data_can() {
    check(
        r#"
//- /src/stub.php
<?php

function hex2bin(string $string): string|false {}

//- /src/app.php
<?php

class Frame
{
    public function run(array $row): void
    {
        $written = hex2bin('400a6375');
        $fromData = hex2bin($row['payload']);
        $odd = hex2bin('400');
        $outside = hex2bin('zz');
        $wri@written@tten;
        $fro@data@mData;
        $od@odd@d;
        $out@outside@side;
    }
}
"#,
        &[
            Query::hover("written at the call", "written"),
            Query::hover("read out of an array", "data"),
            Query::hover("an odd number of digits", "odd"),
            Query::hover("a character outside the alphabet", "outside"),
        ],
        expect![[r#"
            written at the call
            - code: string $written
            - note: assigned
            read out of an array
            - code: string|false $fromData
            - note: assigned
            an odd number of digits
            - code: string|false $odd
            - note: assigned
            a character outside the alphabet
            - code: string|false $outside
            - note: assigned
        "#]],
    );
}
