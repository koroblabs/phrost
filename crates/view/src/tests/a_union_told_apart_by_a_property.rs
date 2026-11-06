//! A union of classes told apart by what a property holds: `is_string($box->value)`.
//!
//! The test is about the property, and it settles the object too -- of `StrBox|IntBox`, only the
//! box whose `value` can be a string passes. Read by the property's *native* type, the one PHP
//! enforces on every write; a docblock's is a claim, and WordPress documents `WP_Post::$post_author`
//! as a `string` while the guard that reads it is there because a REST controller hands it an `int`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_property_kind_test_keeps_the_classes_it_can_be() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

final class StrBox { public string $value = ''; }
final class IntBox { public int $value = 0; }
final class Documented
{
    /** @var string */
    public $value;
}

function run(StrBox|IntBox $box, StrBox|Documented $loose): void
{
    if (is_string($box->value)) {
        $s@string@tring = $box;
    }
    if (!is_string($box->value)) {
        $n@not@ot = $box;
    }
    if (is_int($loose->value)) {
        $d@documented@ocumented = $loose;
    }
}
"#,
        &[
            Query::variable_type("the value is a string", "string"),
            Query::variable_type("the value is not", "not"),
            Query::variable_type("a documented type rules nothing out", "documented"),
        ],
        expect![[r#"
            the value is a string
            - StrBox
            the value is not
            - IntBox
            a documented type rules nothing out
            - Documented
        "#]],
    );
}
