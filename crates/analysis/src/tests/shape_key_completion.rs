//! Offering the keys of a shape inside a subscript that is still being typed.
//!
//! The keys themselves are `shape_keys`' subject. This is about the position: a `[` with no `]` after
//! it does not parse, and what mago recovers from it decides whether there is anything to ask.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn an_unterminated_subscript_offers_the_keys_of_its_shape() {
    check(
        r#"
//- /src/after.php
<?php
namespace App;

class Runner
{
    /** @param array{foo: string, bar: int} $row */
    public function run(array $row): void
    {
        $row[@key@
        $this->run($row);
    }
}
"#,
        &[Query::completion("still being typed", "key")],
        expect![[r#"
            still being typed
            - foo [property]  string  insert `'foo'`
            - bar [property]  int  insert `'bar'`
            - accepting the first leaves `$row['foo'`
        "#]],
    );
}

/// A `[` with nothing after it to resync on is dropped whole by mago, so the shape does not come
/// from the statement -- it comes from the `@var` above it, which lowering keeps as an annotation
/// even when the statement it was written for no longer exists.
///
/// This used to offer nothing, and the fix was assumed to need a second parse of a repaired buffer.
/// It needed the docblock instead: the annotation is what says the name and the shape, and the
/// dropped statement was never the thing holding them.
#[test]
fn a_bracket_at_the_end_of_the_file_still_offers_the_annotated_shape() {
    check(
        r#"
//- /src/top.php
<?php

/** @var array{foo: string, bar: int} $row */
$row[@key@
"#,
        &[Query::completion("from the annotation alone", "key")],
        expect![[r#"
            from the annotation alone
            - foo [property]  string  insert `'foo'`
            - bar [property]  int  insert `'bar'`
            - accepting the first leaves `$row['foo'`
        "#]],
    );
}
