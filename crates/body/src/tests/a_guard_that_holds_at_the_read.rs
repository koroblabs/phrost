//! A guard is complemented for the path that skipped a write, and only if it separates the two.
//!
//! What the third operand of a conditional write's join says is the guard that let control into the
//! write's branch, read backwards: `if ($x === NULL) { $x = 'd'; }` was entered because the name was
//! null, so the path around it is where it was not. That reading needs the guard to be *about* the
//! branch, and the search for it finds whatever narrows the name at the write -- an enclosing guard
//! included:
//!
//! ```php
//! if (!is_array($terms)) {
//!     if (',' !== $comma) { $terms = str_replace($comma, ',', $terms); }
//!     trim($terms, ' ,');
//! }
//! ```
//!
//! `!is_array` holds at the read as surely as at the write, so complementing it says the skipped
//! path held an array -- the one thing the branch both lines are in rules out. `since` is the start
//! of the outermost branch the reader is *not* in, so a guard ending before it encloses both and is
//! left alone.

use expect_test::expect;

use super::check;

#[test]
fn a_guard_enclosing_the_read_as_well_is_not_read_backwards() {
    check(
        r#"<?php
class Repo
{
    public function run(string|array $terms, string $comma): void
    {
        if (!is_array($terms)) {
            if (',' !== $comma) {
                $terms = str_replace($comma, ',', $terms);
            }
            $ter$0ms;
        }
    }
}
"#,
        "terms",
        expect!["str_replace() or $terms before"],
    );
}

/// The guard the reading exists for, so the two are pinned against each other: this one is the
/// write's own branch guard and the read is outside it.
#[test]
fn the_guard_that_let_control_into_the_branch_still_is() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $v): void
    {
        if ($v === NULL) {
            $v = 'default';
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["(string) or $v before minus null"],
    );
}
