//! Flattening happens once per class per view.

use std::sync::Arc;

use phrost_harness::{OpenScope, World};

use crate::IndexView;

#[test]
fn two_asks_about_one_class_share_a_table() {
    // Pointer equality, because that is the claim: not that the two tables agree, but that the
    // second ask did not walk the hierarchy again. A whole-file pass asks about `$this` once per
    // member access, and without this every one of them re-flattens every parent above it.
    let world = World::parse(
        r#"
//- /app.php
<?php
class Base {
    public function fromBase(): void {}
}
class Child extends Base {
    public function own(): void {}
}
"#,
    );
    let file = world.file_id("app.php");
    let open = world.open(OpenScope::CursorFileOnly, Some(file));
    let view = IndexView::in_file(world.skeleton(), &open, file);

    let first = view.members("Child");
    let second = view.members("Child");
    assert!(Arc::ptr_eq(&first, &second));

    // PHP looks classes up case-insensitively, so a different spelling must not build a second one.
    assert!(Arc::ptr_eq(&first, &view.members("child")));

    // And a different class is a different table, not a stale hit.
    assert!(!Arc::ptr_eq(&first, &view.members("Base")));
}
