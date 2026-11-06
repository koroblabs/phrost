//! An unsettled conditional return whose branches share an operand names it once.
//!
//! `class_implements()` is `array<interface-string, interface-string>` for an object and the same or
//! `false` for anything else. Where the call cannot say which, the answer is both branches -- and
//! joined whole the second, a union, read as one operand beside the first: `A|A|false`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_type_both_branches_name_is_named_once() {
    check_view(
        r#"
//- /src/run.php
<?php
namespace App;

/**
 * @return ($subject is object ? array<string, string> : array<string, string>|false)
 */
function implemented(mixed $subject): array|false { return false; }

function run(mixed $value): void
{
    $found = implemented($value);
    $f@found@ound;
}
"#,
        &[Query::variable_type("either branch", "found")],
        expect![[r#"
            either branch
            - array<string, string>|false
        "#]],
    );
}
