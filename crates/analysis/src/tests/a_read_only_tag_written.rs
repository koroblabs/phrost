//! A write to a property a class documents `@property-read`.
//!
//! The tag is lowered as `private(set)`, which is what it means for who may write -- and said so in
//! the message, which named a modifier nobody wrote.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn the_message_names_the_tag() {
    check(
        r#"
//- /src/app.php
<?php

/** @property-read int $count */
class Bag
{
    public function __get($name)
    {
    }

    public function __set($name, $value)
    {
    }
}

function run(Bag $bag): void
{
    $bag->count = 5;
}
"#,
        expect![[r#"
            17:5-17:16 error [readonly-write] Cannot write Bag::$count, which is documented `@property-read`
        "#]],
    );
}
