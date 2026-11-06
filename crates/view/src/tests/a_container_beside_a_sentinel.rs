//! `foreach (preg_split(...) as $piece)` -- iterating `list<string>|false` yields a string.
//!
//! A union's element is the union of its operands' elements, and an operand that has none rules the
//! whole union out: `string|Foo[]` read by index must not answer `Foo`. A `false` or a `null`
//! beside a container is not that, though. It is the call saying it failed or found nothing, and the
//! nullable spelling of exactly this already looked through it -- so `?list<string>` iterated gave a
//! string while `list<string>|false` and `list<string>|null` gave nothing, and every call the loop
//! variable then reached settled no conditional.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_sentinel_beside_a_container_does_not_veto_its_element() {
    check_view(
        r#"
//- /src/run.php
<?php
/**
 * @param list<string>|false $failed
 * @param list<string>|null $missing
 * @param ?list<string> $nullable
 * @param string|list<string> $either
 */
function run($failed, $missing, $nullable, $either): void
{
    foreach ($failed as $one) {
        $o@failed@ne;
    }
    foreach ($missing as $two) {
        $t@missing@wo;
    }
    foreach ($nullable as $three) {
        $th@nullable@ree;
    }
    foreach ($either as $four) {
        $fo@either@ur;
    }
}
"#,
        &[
            Query::variable_type("beside false", "failed"),
            Query::variable_type("beside null", "missing"),
            Query::variable_type("the nullable spelling", "nullable"),
            Query::variable_type("beside a string, which is no sentinel", "either"),
        ],
        expect![[r#"
            beside false
            - string
            beside null
            - string
            the nullable spelling
            - string
            beside a string, which is no sentinel
            - (no type)
        "#]],
    );
}
