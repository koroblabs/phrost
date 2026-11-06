//! `$value ?? ''` over a parameter whose docblock spells the null as one operand of a union.
//!
//! The docblock union arrives nested -- `string|array|null` is `string|(array|null)` to the parser
//! -- so the null is never an operand of the outer union, and taking off only the outer operands
//! left it standing. Laravel's `response($content ?? '')` was reported for a null that `??` had just
//! replaced: fifty such findings on that tree, and eleven more on Symfony and WordPress.

use expect_test::expect;

use crate::tests::{Query, check};

const FIXTURE: &str = r#"
//- /src/A.php
<?php
namespace App;

/** @param string|array|null $doc */
function run(?string $native, $doc = null): void
{
    $a@native@ = $native ?? '';
    $b@doc@ = $doc ?? '';
}
"#;

#[test]
fn a_coalesce_takes_the_null_off_however_the_union_nests() {
    check(
        FIXTURE,
        &[
            Query::hover("a native nullable", "native"),
            Query::hover("a docblock union with null as its last operand", "doc"),
        ],
        expect![[r#"
            a native nullable
            - code: string $a
            - note: assigned
            a docblock union with null as its last operand
            - code: string|array $b
            - note: assigned
        "#]],
    );
}
