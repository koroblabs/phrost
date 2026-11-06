//! A docblock beside a hint that admits `null` keeps the `null`, and the element type beside it.
//!
//! `/** @var DOMNamedNodeMap<DOMAttr> */ public ?DOMNamedNodeMap $attributes;` is the stubs' own
//! spelling: the docblock says what the map holds and nothing about whether there is one. Read as
//! written it either won and lost the `null` or lost and took the element type with it, and the
//! second is what happened -- `foreach ($el->attributes as $attr)` typed `$attr` as a bare `DOMNode`.
//! Settled here because both readers of a declaration take its docblock from lowering.

use expect_test::expect;

use super::check_hir;

#[test]
fn the_null_the_hint_admits_is_put_back_on_the_docblock() {
    check_hir(
        r#"
//- /src/app.php
<?php

class Holder
{
    /** @var Map<Attr> */
    public ?Map $nullable;

    /** @var Map<Attr>|Set<Attr> */
    public Map|Set|null $union;

    /** @var Map<Attr>|null */
    public ?Map $said;

    /** @var Map<Attr> */
    public Map $plain;

    /** @var Map<Attr> */
    public mixed $mixed;

    /**
     * @param list<int> $ids
     * @return list<int>
     */
    public function ids(?array $ids): ?array {}
}
"#,
        expect![[r#"
            class Holder @ 3:7-3:13
              property $nullable: ?Map  public @ 6:17-6:26  [doc]
              property $union: Map|Set|null  public @ 9:25-9:31  [doc]
              property $said: ?Map  public @ 12:17-12:22  [doc]
              property $plain: Map  public @ 15:16-15:22  [doc]
              property $mixed: mixed  public @ 18:18-18:24  [doc]
              method ids(?array $ids): ?array doc: ?array<int, int>  public @ 24:21-24:24  [doc]
        "#]],
    );
}
