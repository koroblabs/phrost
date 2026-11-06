//! An assignment written into a condition, beside a guard about the same name.
//!
//! `if (!empty($ids) && $ids = reset($ids))` is one line doing two things, and the order decides
//! which value each half is about: the guard tested what `$ids` held before, and the assignment
//! replaced it. Composing the two claims anchored the answer on the *name* -- `not null not null
//! $ids` -- with the value the assignment produced nowhere in it, so every reader below the
//! condition saw the parameter's own type.

use expect_test::expect;

use super::check;

#[test]
fn an_assignment_in_the_condition_replaces_what_the_guard_beside_it_tested() {
    check(
        r#"<?php
class Runner {
    public function go(array $ids): void {
        if (!empty($ids) && $ids = reset($ids)) {
            $i$0ds;
        }
    }
}
"#,
        "ids",
        expect!["not null reset()"],
    );
}

/// A guard about a *different* name has nothing to do with the assignment, and composing them was
/// never the question -- this is the control that says the rule above is about the rebinding.
#[test]
fn a_guard_about_another_name_leaves_the_assignment_alone() {
    check(
        r#"<?php
class Runner {
    public function go(array $ids): void {
        if (!empty($ids) && $first = reset($ids)) {
            $fi$0rst;
        }
    }
}
"#,
        "first",
        expect!["not null reset()"],
    );
}
