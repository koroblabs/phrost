//! A write whose branch guard is about something else does not silence the writes beside it.
//!
//! The third operand of the join is what the path that skipped *every* write satisfies, so it
//! satisfies whatever skipping any one of them requires -- a claim true of the wider set is true of
//! the intersection. A write whose guard says nothing about this name therefore contributes nothing
//! and the others still decide; two that say *different* things do end it, because this carries one
//! claim and picking either would state more than the code did.
//!
//! Read as needing every write to agree, one silent write threw the others away. The shape that
//! costs is a nested lazy fill -- `if ($c) { $v = new Fmt(); } else { if (!isset($v)) { $v = new
//! Fmt(); } }` -- where skipping the first write means taking the other arm, which says nothing
//! about `$v`, and skipping the second means `isset` was true, which is the whole answer. Symfony's
//! `FixedWindowLimiter` writes it with `instanceof` instead of `isset`: 21 `possibly-null-receiver`
//! on one method.

use expect_test::expect;

use super::check;

#[test]
fn a_silent_write_leaves_the_other_writes_claim_standing() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $v, bool $c): void
    {
        if ($c) {
            $v = 'left';
        } else {
            if (!isset($v)) {
                $v = 'right';
            }
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["(string) or (string) or $v before minus null"],
    );
}

/// Two writes whose guards prove different things, on the one way around both: there `isset` held
/// and `is_int` did not, and the first of the two is the one a reader needs -- the value was there.
#[test]
fn two_guards_on_the_one_way_around_both_writes_held_together() {
    check(
        r#"<?php
class Repo
{
    public function run(?string $v, bool $c): void
    {
        if (!isset($v)) {
            $v = 'left';
        } elseif (is_int($v)) {
            $v = 'right';
        }
        $$0v;
    }
}
"#,
        "v",
        expect!["(string) or (string) or $v before minus null"],
    );
}
