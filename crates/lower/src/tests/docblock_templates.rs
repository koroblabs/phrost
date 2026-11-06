//! `@template` parameters on a class-like declaration.
//!
//! The only place PHP lets a container say what it holds, and the reason iterating a Drupal field
//! list yielded nothing: `FieldItemListInterface` is `@template T of FieldItemInterface` and the
//! language itself says nothing at all.

use expect_test::expect;

use crate::tests::check_hir;

#[test]
fn a_bounded_template_keeps_its_bound_and_its_range() {
    check_hir(
        r#"
//- /src/Items.php
<?php
namespace App;

class Item {}

/**
 * A list of items.
 *
 * @template T of Item
 * @template-covariant K of int|string
 * @template U
 */
interface Items {}
"#,
        expect![[r#"
            namespace App @ 2:11-2:14
              class Item @ 4:7-4:11
              interface Items @ 13:11-13:16  [doc]
                template T of Item @ 9:14-9:15
                template K of int|string @ 10:24-10:25
                template U @ 11:14-11:15
        "#]],
    );
}

#[test]
fn a_class_with_no_template_tag_carries_none() {
    check_hir(
        r#"
//- /src/Plain.php
<?php
namespace App;

/** Just prose. */
class Plain {}
"#,
        expect![[r#"
            namespace App @ 2:11-2:14
              class Plain @ 5:7-5:12  [doc]
        "#]],
    );
}
