//! A local built by a literal and then written into, read as a whole.
//!
//! An element write is not a binding, so the literal used to be the whole answer:
//! `$args = ['--force']; $args[] = $path;` read as the one entry it started with, and a type narrower
//! than the truth reports correct code. The writes are folded in now -- see `locals/filled.rs` --
//! and each rule says no more than the write does.

use expect_test::expect;

use crate::tests::{Query, check};

const FIXTURE: &str = r#"
//- /src/A.php
<?php
namespace App;

function run(bool $flag, string $path, int $n, string $key): void
{
    $args = ['--force'];
    $args[] = $path;
    $a@appended@ = $args;

    $row = ['id' => 1];
    $row['name'] = 'x';
    if ($flag) {
        $row['id'] = null;
    }
    $b@keyed@ = $row;

    $list = [1, 2];
    if ($flag) {
        $list[] = 'three';
    }
    $c@maybe_appended@ = $list;

    $map = ['a' => 1];
    $map[$key] = 2.5;
    $d@computed@ = $map;

    $tree = ['children' => [], 'id' => $n];
    $tree['children'][] = $path;
    $e@through@ = $tree;

    $stack = [$n];
    foreach ([1, 2] as $i) {
        $f@carried@ = $stack;
        $stack[] = 'pushed';
    }

    $big = ['a' => 1];
    $big['b'] = 1; $big['c'] = 1; $big['d'] = 1; $big['e'] = 1; $big['f'] = 1;
    $big['g'] = 1; $big['h'] = 1; $big['i'] = 1; $big['j'] = 1; $big['k'] = 1;
    $big['l'] = 1; $big['m'] = 1; $big['n'] = 1; $big['o'] = 1; $big['p'] = 1;
    $big['q'] = 1;
    $g@overflowing@ = $big;
}
"#;

#[test]
fn the_writes_since_the_literal_are_part_of_what_it_holds() {
    check(
        FIXTURE,
        &[
            Query::hover("an append takes the next integer key", "appended"),
            Query::hover(
                "a key written, and one rewritten in a branch beside what it held",
                "keyed",
            ),
            Query::hover(
                "an append that may not have run grows a list",
                "maybe_appended",
            ),
            Query::hover("a key nothing names makes a map", "computed"),
            Query::hover(
                "a write through a slot leaves an array in that slot",
                "through",
            ),
            Query::hover(
                "a write a loop carries round to the top of the pass",
                "carried",
            ),
            Query::hover(
                "past the cap the record is an array and no more",
                "overflowing",
            ),
        ],
        expect![[r#"
            an append takes the next integer key
            - code: array{0: non-falsy-string, 1: string} $a
            - note: assigned
            a key written, and one rewritten in a branch beside what it held
            - code: array{id: ?int, name: non-falsy-string} $b
            - note: assigned
            an append that may not have run grows a list
            - code: array<int|non-falsy-string> $c
            - note: assigned
            a key nothing names makes a map
            - code: array<int|string, int|float> $d
            - note: assigned
            a write through a slot leaves an array in that slot
            - code: array{children: array, id: int} $e
            - note: assigned
            a write a loop carries round to the top of the pass
            - code: array<int|non-falsy-string> $f
            - note: assigned
            past the cap the record is an array and no more
            - code: array $g
            - note: assigned
        "#]],
    );
}
