//! Whether a member table kept between requests is dropped when a class it was built from changes.
//!
//! The memo answers every later request from what an earlier one built, so the one thing that can
//! go wrong is a stale table: a parent gains a method and a child's table, built before, still
//! says it has none. The rule is that `install` and `retract` evict every table whose contributors
//! include a class the file declares -- and that an unrelated class's table survives, or the memo
//! would be emptied by every keystroke and buy nothing.

use phrost_sema::{FlattenMemo as _, MemberKind, flatten_class_with};

use super::{Order, build, fixture_files};

const FIXTURE: &str = r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public function first(): int { return 1; }
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base
{
    public function own(): int { return 2; }
}

//- /src/Other.php
<?php
namespace App;

class Other
{
    public function alone(): int { return 3; }
}
"#;

const BASE_GROWN: &str = r#"<?php
namespace App;

class Base
{
    public function first(): int { return 1; }
    public function second(): int { return 2; }
}
"#;

#[test]
fn a_table_is_dropped_when_a_class_it_was_built_from_changes_and_kept_otherwise() {
    let mut built = build(&fixture_files(FIXTURE), Order::ByPath);

    // One walk fills the memo with the child and, on the way, the base.
    let child = flatten_class_with(&built.skeleton, "App\\Child", &built.skeleton);
    assert!(child.find(MemberKind::Method, "first").is_some());
    flatten_class_with(&built.skeleton, "App\\Other", &built.skeleton);
    assert!(built.skeleton.flattened("app\\child").is_some());
    assert!(built.skeleton.flattened("app\\base").is_some());
    assert!(built.skeleton.flattened("app\\other").is_some());

    // The base gains a method: every table built from it goes, the unrelated one stays.
    let base = built
        .file_id("src/Base.php")
        .expect("the fixture declares it");
    built.skeleton.ingest(
        base,
        &super::fixture_path("src/Base.php"),
        phrost_model::SourceOrigin::User,
        BASE_GROWN,
    );
    assert!(built.skeleton.flattened("app\\child").is_none());
    assert!(built.skeleton.flattened("app\\base").is_none());
    assert!(built.skeleton.flattened("app\\other").is_some());

    // And the next walk sees the new member rather than a remembered table without it.
    let child = flatten_class_with(&built.skeleton, "App\\Child", &built.skeleton);
    assert!(child.find(MemberKind::Method, "second").is_some());

    // Removing the file evicts too.
    built.skeleton.remove(base);
    assert!(built.skeleton.flattened("app\\child").is_none());
    assert!(built.skeleton.flattened("app\\other").is_some());
}
