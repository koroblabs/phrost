//! An array a falsy guard holds, which can only be `[]`.
//!
//! `[]` is PHP's one falsy array, so inside `if ($rows === [])`, `if (empty($rows))` or `if
//! (!$rows)` the array is `array{}` -- and a record with no fields is how that is spelled. It used
//! to stay the declared `array`, which was wider than true and let `$rows[0]` through.
//!
//! Only where the guard was about the value itself. A guard on a call is read as a slot, so
//! `if (!$this->find(A)) { ... } elseif ($r = $this->find(B))` hands the second call the first
//! one's falsehood when the step cannot tell `A` from `B` -- harmless while the answer was `array`,
//! and an empty array that is not empty once it was spelled out.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_falsy_array_is_the_empty_one() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{
    /** @return list<int> */
    public function find(string $kind): array { return []; }

    /** @param list<int> $rows */
    public function run(array $rows, ?array $maybe): void
    {
        if ($rows === []) {
            $i@identical@dentical = $rows;
        }
        if (empty($rows)) {
            $e@empty@mpty = $rows;
        }
        if (!$maybe) {
            $m@maybe@aybe = $maybe;
        }
        if ($r = $this->find('a')) {
            return;
        } elseif ($r = $this->find('b')) {
            $c@call@all = $r;
        }
        $f@fallback@allback = $rows ?: 'none';
    }
}
"#,
        &[
            Query::variable_type("compared with []", "identical"),
            Query::variable_type("empty()", "empty"),
            Query::variable_type("a nullable tested falsy", "maybe"),
            Query::variable_type("a call read as a slot", "call"),
            Query::variable_type("?: never hands back the empty one", "fallback"),
        ],
        expect![[r#"
            compared with []
            - array{}
            empty()
            - array{}
            a nullable tested falsy
            - ?array{}
            a call read as a slot
            - array<int, int>
            ?: never hands back the empty one
            - array<int, int>|non-falsy-string
        "#]],
    );
}
