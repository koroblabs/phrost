//! What a property or a parameter holds when its docblock refines a hint that admits `null`.
//!
//! Lowering puts the `null` back on the docblock (see the test of the same name there); this is the
//! half a reader sees. The stubs' `DOMNode::$attributes` is the case that found it: its element type
//! was lost to the hint, so `foreach ($el->attributes as $attr)` read `$attr` as a bare `DOMNode`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_element_type_and_the_null_both_survive() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Attr {}
/** @template T */
class Map {}

class Holder
{
    /** @var Map<Attr> */
    public ?Map $nullable;
    /** @var Map<Attr> */
    public Map $plain;
    /** @var Map<Attr> */
    public mixed $mixed;
}

/** @param list<Attr> $ids */
function run(Holder $h, ?array $ids): void
{
    $n@nullable@ullable = $h->nullable;
    $p@plain@lain = $h->plain;
    $m@mixed@ixed = $h->mixed;
    $i@param@ds = $ids;
}
"#,
        &[
            Query::variable_type(
                "a nullable hint keeps its null beside the docblock",
                "nullable",
            ),
            Query::variable_type("a hint that admits none adds none", "plain"),
            Query::variable_type("`mixed` is a hint saying nothing, not a null", "mixed"),
            Query::variable_type("a parameter's docblock beside `?array`", "param"),
        ],
        expect![[r#"
            a nullable hint keeps its null beside the docblock
            - ?Map<Attr>
            a hint that admits none adds none
            - Map<Attr>
            `mixed` is a hint saying nothing, not a null
            - Map<Attr>
            a parameter's docblock beside `?array`
            - ?array<int, Attr>
        "#]],
    );
}
