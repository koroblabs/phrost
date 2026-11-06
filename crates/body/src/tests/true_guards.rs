//! `$x === true`, which narrows exactly as `$x === false` does and did not.
//!
//! The sibling of [`super::false_guards`], and it exists because that file had no sibling for a
//! year. `false` earned its own node because the standard library reports "not found" with it --
//! `strpos()`, `strtotime()`, `array_search()` -- and `true` has no such use, so nobody wrote the
//! other half of the rule.
//!
//! What does write it is a map whose value means *all of them*: `['order' => ['a', 'b'], 'user' =>
//! TRUE]` is an ordinary Drupal idiom, read as `TRUE === $map[$k] || in_array($b, $map[$k])`. The
//! `||` rules the `true` out of its right-hand side, and until the node carried which boolean was
//! tested it ruled out nothing -- so the value still read as `string[]|true` and passing it to a
//! parameter that takes an array was reported on code the author had already guarded.
//!
//! Found by giving a record literal a type: the narrowing gap had always been there and nothing
//! could see it while the map itself was a bare `array`.

use expect_test::expect;

use super::check;

#[test]
fn a_true_identity_removes_the_true_from_a_union() {
    check(
        r#"<?php
class Repo
{
    /** @param string[]|true $value */
    public function run($value): void
    {
        if (true === $value) {
            return;
        }
        $val$0ue;
    }
}
"#,
        "value",
        expect!["not (true) $value"],
    );
}

#[test]
fn a_true_identity_on_a_guard_is_read_at_its_own_polarity() {
    check(
        r#"<?php
class Repo
{
    public function run($value): void
    {
        if (true === $value instanceof Alpha) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["instanceof Alpha $value"],
    );
}
