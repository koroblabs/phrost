//! An out-parameter is written wherever the call is, and eleven shapes are not eleven statements.
//!
//! `passed_to_calls` sees only a call written as a statement of its own, which its own doc justifies
//! for `@assert` -- `$this->assertNotNull($x);` is written for its effect and nothing else. The
//! by-reference reader shared that walk, so of the eleven ways one function is written, **one**
//! answered:
//!
//! ```text
//! preg_match($p, $s, $m);            string[]
//! if (preg_match($p, $s, $m))        nothing
//! $ok = preg_match($p, $s, $m)       nothing
//! ...
//! ```
//!
//! The first is the shape a probe reaches for and the second is the shape everybody writes, which is
//! how `@param-out` came to be measured and reported as working. On the reference project all three
//! real capture sites are `if (preg_match(...))` and `if (!preg_match(...))`: nought of three.
//!
//! # Why two walks and not one wide one
//!
//! Widening the shared walk cost `phrost check` 2.42 s -> 3.15 s on that project, +30 %, because
//! `contract_narrowed` asks it about every value whose declaration was vague and that is most of
//! them. `passed_to_any_call` is the wide one and is reached only from the `ByReference` root, which
//! exists precisely for a name nothing else bound -- rare, and the only place the width is needed.
//! Back to 2.46 s with the split.
//!
//! # The `else` branch
//!
//! `if (preg_match($p, $s, $m)) {} else { $m }` answers `string[]`, and that is right: PHP writes the
//! empty array whether or not the pattern matched. Making the tag visible only in the branch the
//! match selected would give a type narrower than the truth, which is the trade this project refuses
//! everywhere else.
//!
//! One shape is unsound and kept: under a short-circuit, `if ($f && preg_match(..., $m)) {} else {
//! $m }` claims a type where the call may not have run. Dropping calls under `&&` would also lose the
//! sound and far commoner `if ($f && preg_match(..., $m)) { $m }`.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn every_position_a_call_can_be_written_in_binds_its_out_parameter() {
    check(
        r#"
//- /src/app.php
<?php

/**
 * A local stand-in for `preg_match`, because this harness indexes no standard library: the test is
 * about the *positions* a call can be written in, and using a stub would make it about the stub.
 *
 * @param-out array<int, string> $matches
 */
function matched(string $pattern, string $subject, &$matches = []): bool {}

function run(string $s, bool $f, array $xs): void
{
    matched('/x/', $s, $a);
    $a@statement@;

    if (matched('/x/', $s, $b)) {
        $b@in_if@;
    }

    if (!matched('/x/', $s, $c)) {
        $c@negated@;
    }

    if (matched('/x/', $s, $d)) {
    } else {
        $d@in_else@;
    }

    if (matched('/x/', $s, $e)) {
    }
    $e@after_if@;

    $ok = matched('/x/', $s, $g);
    $g@assigned@;

    if (matched('/x/', $s, $h) === 1) {
        $h@compared@;
    }

    while (matched('/x/', $s, $i)) {
        $i@in_while@;
    }

    if ($f && matched('/x/', $s, $j)) {
        $j@short_circuit@;
    }

    foreach ($xs as $x) {
        matched('/x/', $s, $k);
        $k@in_loop@;
    }
}
"#,
        &[
            Query::hover("a statement of its own", "statement"),
            Query::hover("in the condition of an if", "in_if"),
            Query::hover("in a negated condition", "negated"),
            Query::hover("in the else of that if", "in_else"),
            Query::hover("after the whole if", "after_if"),
            Query::hover("inside an assignment", "assigned"),
            Query::hover("inside a comparison", "compared"),
            Query::hover("in a while condition", "in_while"),
            Query::hover("guarded by a short circuit", "short_circuit"),
            Query::hover("in a loop body", "in_loop"),
        ],
        expect![[r#"
            a statement of its own
            - code: array<int, string> $a
            - note: assigned
            in the condition of an if
            - code: array<int, string> $b
            - note: assigned
            in a negated condition
            - code: array<int, string> $c
            - note: assigned
            in the else of that if
            - code: array<int, string> $d
            - note: assigned
            after the whole if
            - code: array<int, string> $e
            - note: assigned
            inside an assignment
            - code: array<int, string> $g
            - note: assigned
            inside a comparison
            - code: array<int, string> $h
            - note: assigned
            in a while condition
            - code: array<int, string> $i
            - note: assigned
            guarded by a short circuit
            - code: array<int, string> $j
            - note: assigned
            in a loop body
            - code: array<int, string> $k
            - note: assigned
        "#]],
    );
}
