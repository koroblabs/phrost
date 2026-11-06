//! `if (!is_string($x)) { $x = 'default'; }` -- the branch that normalises rather than alternates.
//!
//! What the skipped path knows is the *negation of the guard*, and for an exact predicate that is
//! precise: `is_string` has no third answer, so around a branch entered because the value was not a
//! string, it was one. The vocabulary carried only the other polarity, on a stated reason -- "only
//! the negative direction" -- which is right for truthiness (falsy is `0` and `''`, so the mirror
//! over-claims) and wrong for a predicate.
//!
//! Read at the body layer because the claim is the tree's: which guard let control into the write,
//! and at what polarity. What the claim *does* to a type is the view's, and is tested there.

use expect_test::expect;

use super::check;

#[test]
fn a_negated_predicate_says_the_skipped_path_was_of_that_kind() {
    check(
        r#"<?php
class Repo
{
    /** @return string|false */
    public function encode(): string|false
    {
        return '';
    }

    public function run(): void
    {
        $b = $this->encode();
        if (!is_string($b)) {
            $b = 'null';
        }
        $$0b;
    }
}
"#,
        "b",
        expect!["(string) or $b before being string"],
    );
}

/// The other polarity, which was already carried: the branch ran because it *was* of the kind.
#[test]
fn a_plain_predicate_says_the_skipped_path_was_not() {
    check(
        r#"<?php
class Repo
{
    public function run(int|string $id): void
    {
        if (is_int($id)) {
            $id = 'n' . $id;
        }
        $$0id;
    }
}
"#,
        "id",
        expect!["(non-falsy-string) or $id before minus int"],
    );
}

/// Truthiness keeps its refusal, and this is the fixture that says why it must: the skipped path is
/// where the value was *truthy*, and claiming it was of one kind would be a guess. What it does
/// carry is the falsehood, which rules out `false` as well as null.
#[test]
fn truthiness_says_falsehood_and_no_more() {
    check(
        r#"<?php
class Repo
{
    /** @return array|false */
    public function rows(): array|false
    {
        return [];
    }

    public function run(): void
    {
        $r = $this->rows();
        if (!$r) {
            $r = [];
        }
        $$0r;
    }
}
"#,
        "r",
        expect!["(array) or $r before minus falsehood"],
    );
}
