//! An assignment the cursor sits at the end of has not happened yet.
//!
//! `$foobar = $foobar->meth` with nothing typed after it, and completion asked after `meth`: the
//! write to `$foobar` on that line ends exactly at the cursor, and a reach test written as "the
//! write ends after the position" let it through -- so the name resolved through its own unfinished
//! value and completion offered nothing. With a character after the cursor, or the cursor one byte
//! to the left, the same line worked, which is how it read as a completion bug rather than a reach
//! one. The write is skipped when the position is anywhere inside it, its end included.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn completing_the_receiver_of_a_self_assignment_reads_the_write_before_it() {
    check(
        r#"
//- /src/app.php
<?php

class Foobar
{
    public function method1(): void {}
}

$foobar = new Foobar();
$foobar = $foobar->meth@here@"#,
        &[Query::completion(
            "at the end of the unfinished line",
            "here",
        )],
        expect![[r#"
            at the end of the unfinished line
            - method1 [method]  : void  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `$foobar = $foobar->method1`
        "#]],
    );
}
