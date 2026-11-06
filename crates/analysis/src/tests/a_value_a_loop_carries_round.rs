//! A write below a read, in the same loop body, ran on the pass before.
//!
//! ```php
//! $lowest = NULL;
//! foreach ($grades as $grade) {
//!     if ($lowest === NULL || $grade->min() < $lowest->min()) {
//!         $lowest = $grade;
//!     }
//! }
//! ```
//!
//! The read of `$lowest` is above the only write that gives it a value, so reading the writes before
//! the position answered *certainly null* -- and the guard beside it then had nothing left to
//! narrow. Every find-the-best loop in PHP is written this way. Found by `compare-lsp` against
//! intelephense on the reference project.
//!
//! Always a *join* and never a replacement: the first pass reaches the read with whatever the name
//! held before the loop, so `$lowest` is `?Grade` and not `Grade`.
//!
//! # What a carried write is not
//!
//! Three refusals, each paid for by a corpus:
//!
//! - **Anything that rebinds the name between the loop's start and the read** kills it -- a certain
//!   write at the top of the body, or the loop's own target. It ran *after* the carried one.
//! - **A value that depends on what the loop also writes** is a fixed point rather than a value
//!   arriving from the pass before: `$input = substr($input, 3)` depends on itself and
//!   `$dir = $renamed . SEP` beside `$renamed = substr_replace($dir, ...)` depends on itself one
//!   variable away. Joining either resolves the name through the loop again until the budget stops.
//! - **An element write** never counts, because what it settles is that the write *created* the
//!   variable, and a carried write is not the first one.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_write_below_the_read_joins_what_the_name_held_before_the_loop() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Grade
{
    public function min(): int
    {
        return 0;
    }
}

class Reader
{
    /** @param list<Grade> $grades */
    public function run(array $grades): void
    {
        $lowest = NULL;
        foreach ($grades as $grade) {
            $at@carried@Top = $lowest;
            if ($lowest === NULL || $grade->min() < $lowest->min()) {
                $lowest = $grade;
            }
            $be@guarded@low = $lowest;
        }
        $af@after@ter = $lowest;
    }
}
"#,
        &[
            Query::hover("at the top of the body, above the write", "carried"),
            Query::hover("past the guard that wrote it", "guarded"),
            Query::hover("after the loop", "after"),
        ],
        expect![[r#"
            at the top of the body, above the write
            - code: ?Grade $atTop
            - note: assigned
            past the guard that wrote it
            - code: Grade $below
            - note: assigned
            after the loop
            - code: ?Grade $after
            - note: assigned
        "#]],
    );
}

/// The refusals. Each of these carries nothing, and each would widen without bound if it did.
#[test]
fn a_carried_write_that_depends_on_the_loop_carries_nothing() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Reader
{
    public function refines(string $input): void
    {
        while ($input !== '') {
            $re@refined@ad = $input;
            $input = substr($input, 3);
        }
    }

    /** @param list<string> $names */
    public function rebound(array $names): void
    {
        foreach ($names as $name) {
            $ta@target@ken = $name;
            $name = 1;
        }
    }

    /** @param list<string> $rows */
    public function intoASlot(array $rows): void
    {
        $seen = 'x';
        foreach ($rows as $row) {
            $sl@slot@ot = $seen;
            $seen[0] = $row;
        }
    }
}
"#,
        &[
            Query::hover("a value that reads itself", "refined"),
            Query::hover("the loop's own target", "target"),
            Query::hover("an element write below", "slot"),
        ],
        expect![[r#"
            a value that reads itself
            - code: non-empty-string $read
            - note: assigned
            the loop's own target
            - code: string $taken
            - note: assigned
            an element write below
            - code: non-falsy-string $slot
            - note: assigned
        "#]],
    );
}
