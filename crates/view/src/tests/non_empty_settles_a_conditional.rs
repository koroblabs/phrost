//! `reset()` and its family, under a guard that says the array holds something.
//!
//! Their stub is written `($array is non-empty-list ? V : ($array is non-empty-array ? V : V|false))`,
//! and the test was refused at lowering as a claim about what the array *contains*. It is not: an
//! empty array is falsy in PHP, so "not empty" is exactly "truthy", and that is a claim the guard
//! machinery already makes. Without it `reset($rows)` under `if (empty($rows)) { return; }` answered
//! `V|false` for a value that cannot be absent, and every call had to be guarded twice.

use expect_test::expect;

use super::{Query, check_view};

const RESET: &str = r#"
//- /stubs/standard.php  stub
<?php
/**
 * @template K
 * @template V
 * @param array<K, V> $array
 * @return ($array is non-empty-list ? V : ($array is non-empty-array ? V : V|false))
 */
function reset(object|array &$array): mixed {}
"#;

#[test]
fn a_guard_that_rules_out_the_empty_array_rules_out_the_failure() {
    check_view(
        &[
            RESET,
            r#"
//- /src/Runner.php
<?php
namespace App;

class Doc {}

class Runner
{
    /** @param Doc[] $docs */
    public function guarded(array $docs): void
    {
        if (empty($docs)) {
            throw new \RuntimeException('none');
        }
        $f@guarded@irst = reset($docs);
    }

    /** @param Doc[] $docs */
    public function truthy(array $docs): void
    {
        if ($docs) {
            $f@truthy@irst = reset($docs);
        }
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("after `empty()` threw", "guarded"),
            Query::variable_type("inside a truthy branch", "truthy"),
        ],
        expect![[r#"
            after `empty()` threw
            - Doc
            inside a truthy branch
            - Doc
        "#]],
    );
}

/// `count($rows) === 0` is `empty($rows)` written out, and PHP's own way of asking it of a
/// `Countable`. All five spellings anybody writes, from either side of the operator.
#[test]
fn a_count_compared_against_zero_says_the_same_thing() {
    check_view(
        &[
            RESET,
            r#"
//- /stubs/countable.php  stub
<?php
function count(\Countable|array $value, int $mode = 0): int {}

//- /src/Runner.php
<?php
namespace App;

class Doc {}

class Runner
{
    /** @param Doc[] $docs */
    public function returns_early(array $docs): void
    {
        if (count($docs) === 0) {
            return;
        }
        $f@early@irst = reset($docs);
    }

    /** @param Doc[] $docs */
    public function tests_for_some(array $docs): void
    {
        if (count($docs) > 0) {
            $f@some@irst = reset($docs);
        }
    }

    /** @param Doc[] $docs */
    public function tests_yoda(array $docs): void
    {
        if (0 !== count($docs)) {
            $f@yoda@irst = reset($docs);
        }
    }

    /** @param Doc[] $docs */
    public function counts_to_three(array $docs): void
    {
        if (count($docs) === 3) {
            $f@three@irst = reset($docs);
        }
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("`=== 0` and an early return", "early"),
            Query::variable_type("`> 0`", "some"),
            Query::variable_type("`0 !== count(...)`", "yoda"),
            Query::variable_type("a count that is not against zero", "three"),
        ],
        expect![[r#"
            `=== 0` and an early return
            - Doc
            `> 0`
            - Doc
            `0 !== count(...)`
            - Doc
            a count that is not against zero
            - Doc|false
        "#]],
    );
}

/// With nothing ruling the empty array out, the failure branch is still what the declaration says.
///
/// The half that keeps the rule honest: `reset([])` really is `false`, and a guard is the only
/// thing that can say otherwise.
#[test]
fn an_unguarded_array_keeps_the_failure_branch() {
    check_view(
        &[
            RESET,
            r#"
//- /src/Runner.php
<?php
namespace App;

class Doc {}

class Runner
{
    /** @param Doc[] $docs */
    public function unguarded(array $docs): void
    {
        $f@plain@irst = reset($docs);
    }

    /** @param Doc[] $docs */
    public function guarded_on_something_else(array $docs, bool $when): void
    {
        if ($when) {
            $f@elsewhere@irst = reset($docs);
        }
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("no guard at all", "plain"),
            Query::variable_type("a guard about something else", "elsewhere"),
        ],
        expect![[r#"
            no guard at all
            - Doc|false
            a guard about something else
            - Doc|false
        "#]],
    );
}
