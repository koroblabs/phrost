//! What a normalising branch leaves behind, as a type.
//!
//! The body layer says which guard let control into the write and at what polarity; this is the
//! other half -- what that claim does to the value on the path that skipped the branch.
//!
//! `$b = json_encode(...); if (!is_string($b)) { $b = 'null'; }` is how PHP normalises a
//! `string|false`, and reading the skipped path as "unproved" put the `false` straight back. Nine
//! `incompatible-argument` errors on one small project, every one on a line that had just handled
//! the case -- and invisible on the only project the checker had been measured against, because
//! Drupal does not `declare(strict_types=1)` and weak mode coerces a `false` to `''`.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /src/Encoder.php
<?php
namespace App;

class Encoder
{
    /** @return string|false */
    public function encode(): string|false
    {
        return '';
    }

    /** @return array|false */
    public function rows(): array|false
    {
        return [];
    }

    public function negatedPredicate(): void
    {
        $b = $this->encode();
        if (!is_string($b)) {
            $b = 'null';
        }
        $@predicate@b;
    }

    public function inTheElse(): void
    {
        $b = $this->encode();
        if (is_string($b)) {
        }
        else {
            $b = 'null';
        }
        $@otherwise@b;
    }

    public function truthy(): void
    {
        $r = $this->rows();
        if (!$r) {
            $r = [];
        }
        $@truthy@r;
    }
}
"#;

/// All three spellings a normalising branch is written in, and the type each leaves.
///
/// The truthiness one is here beside the predicates deliberately: its claim is weaker -- falsehood
/// rather than a kind -- and it still has to remove the `false`, which is what the standard library
/// means by failure and what the branch was written to replace.
#[test]
fn a_normalising_branch_leaves_the_type_it_was_written_to_produce() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("`!is_string(...)` then a write", "predicate"),
            Query::variable_type("the same, written as an else", "otherwise"),
            Query::variable_type("`!$r` then a write", "truthy"),
        ],
        expect![[r#"
            `!is_string(...)` then a write
            - string
            the same, written as an else
            - string
            `!$r` then a write
            - array
        "#]],
    );
}
