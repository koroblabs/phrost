//! `$item = $item->resource;` completed at the end of the name.
//!
//! The cursor there is the end of the assignment, and reading the receiver at the cursor read it
//! *after* the write: whatever narrowed `$item` was undone by the value replacing it, so the list was
//! empty while goto on the same name answered. The receiver is read where the name starts.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn the_receiver_is_read_before_the_write_it_is_part_of() {
    check(
        r#"
//- /src/A.php
<?php
namespace App;

class Wrapper
{
    public Inner $inner;
}

class Inner {}

class A
{
    public function go(object $item): void
    {
        if ($item instanceof Wrapper) {
            $item = $item->inner@here@;
        }
    }
}
"#,
        &[Query::completion("at the end of the name", "here")],
        expect![[r#"
            at the end of the name
            - inner [property]  Inner
            - ... and the client must ask again
            - accepting the first leaves `$item = $item->inner;`
        "#]],
    );
}
