//! What a by-reference argument leaves in the caller's variable.
//!
//! Three claims, ranked. A `@param-out` tag is somebody stating what comes back out and replaces
//! what the caller knew. A `mixed` parameter is the declaration saying nothing about what it
//! accepts, so nothing bounds what was written and what the caller knew is stale. Anything else is
//! a bound on what the callee *accepts* -- PHP's parameters are contravariant -- so it is asked only
//! whether it contradicts the caller, and the caller's own type stands where it does not.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn an_untyped_by_reference_parameter_still_says_nothing() {
    // `preg_match`'s `$matches` is the case this mechanism exists for -- the variable exists only
    // because of the call -- and against the real stubs it now answers `string[]`, read off their
    // `@param-out array<string>`. It is unanswered *here*, and the reason is the harness rather than
    // the feature: this fixture indexes no standard library, so `preg_match` resolves to no
    // declaration and there is no parameter to read a tag from.
    //
    // Which is what it is worth keeping: a by-reference call whose callee cannot be resolved must
    // leave the variable with no type rather than guess one. The positions the tag *is* read in are
    // pinned against a local declaration in `an_out_parameter_in_every_position`.
    check(
        r#"
//- /src/app.php
<?php

function run(string $text): void
{
    preg_match('/x/', $text, $matches);
    $mat@matches@ches;
}
"#,
        &[Query::hover("written by the call", "matches")],
        expect![[r#"
            written by the call
            - code: $matches
            - note: assigned
            - note: no declared type
        "#]],
    );
}

/// `mixed &$x` invalidates what the caller knew, and keeping it is the unsound direction.
///
/// The asymmetry against the union below is the point. A union is an upper bound on what the callee
/// *accepts*, so believing it widens and *adds* findings. `mixed` says anything of any type may come
/// back out, so keeping the caller's own type leaves a claim the call has already broken -- and the
/// break shows up as a report. On one real project `assertSeverityLevel(mixed &$level)` narrows its
/// argument internally, the caller's `Level|int` was stale by the time it reached a `Level` property,
/// and the assignment was reported. PHPStan says nothing there.
///
/// Widening to `mixed` can only remove a finding, since every check treats `mixed` as no claim --
/// which is what makes this safe without reading the callee's body.
///
/// # And hover now says so too
///
/// It used to say `Level|int` here, and the same for a *typed* by-reference parameter: `$rows =
/// "text"` then `fills(array &$out)` hovered `string` before and after while every check said
/// `array`. Both readers reach the same refinement -- `IndexView::contract_narrowed`, which walks
/// `passed_to_calls` -- and hover handed it the wrong **subject**. That walk matches a call's
/// argument by comparing written paths, so it fires only for the local's own path; hover handed it
/// `LocalBinding::value`, the path of what the binding holds, which matches nothing. The position was
/// not the cause: refining at the cursor and keeping the value's path was measured and changed
/// nothing. `phrost_body::local_path` is now the one path a reader outside that crate builds, so
/// there is no second spelling of the subject to get wrong.
#[test]
fn a_mixed_by_reference_parameter_takes_away_what_the_caller_knew() {
    check(
        r#"
//- /src/app.php
<?php

enum Level: int
{
    case Low = 1;
}

class Holder
{
    private Level $level;

    private function normalise(mixed &$level): void
    {
        if (is_int($level)) {
            $level = Level::from($level);
        }
    }

    public function cases(Level|int $a): void
    {
        $this->normalise($a);
        $this->level = $a;
        $a@after@;
    }
}
"#,
        &[Query::hover("after the call", "after")],
        expect![[r#"
            after the call
            - code: mixed $a
            - note: parameter
        "#]],
    );
}

#[test]
fn a_union_by_reference_parameter_leaves_what_the_caller_knew() {
    // `array_walk(object|array &$array)` accepts either and writes neither: the union is an upper
    // bound on what it takes, not a claim about what it left. Believing it widened a plain `array`
    // to `object|array`, and every later `array_shift($rows)` was reported for the object branch
    // that cannot be there.
    check(
        r#"
//- /src/app.php
<?php

function run(array $rows): void
{
    array_walk($rows, static function (&$x): void {});
    $ro@rows@ws;
}
"#,
        &[Query::hover("still what the parameter declared", "rows")],
        expect![[r#"
            still what the parameter declared
            - code: array $rows
            - note: parameter
        "#]],
    );
}

/// A signature is a bound on what the callee accepts, so what the caller knew survives it.
///
/// PHP's parameters are contravariant: `sort(array &$array)` says it takes an array and nothing
/// more, and a caller holding a `list<Item>` satisfies that bound. Reading the declaration as "an
/// array is what is there now" threw the element type away at 36 sites on one real checkout --
/// `sort` 11, `usort` 11, `array_splice` 6, `uasort` 4, `rsort` 2, `array_walk` 1, `array_push` 1.
/// Never a false positive, since wider than the truth reports nothing; it is precision that existed
/// and stopped existing, which is the same loss as a `T[]` leaking into a render, arriving through
/// inference instead of through a renderer.
///
/// The two directions are one fixture because the rule is one comparison. `Fit::Rejected` is the
/// test rather than `Fit::Accepted`, and `sort` is why: it writes `@param array<T>` beside its
/// `array` hint, a bare container is a hint that says nothing, so the effective type is `array<T>`
/// with `T` bound by nothing -- and a `list<Item>` against that is `Fit::Unknown`. Demanding
/// `Accepted` would have kept the bug on the function that exposed it.
#[test]
fn a_typed_by_reference_parameter_keeps_what_the_caller_knew_unless_it_contradicts_it() {
    check(
        r#"
//- /src/app.php
<?php

class Item {}

function shuffled(array &$items): void {}

/**
 * @param list<Item> $items
 */
function run(array $items): void
{
    shuffled($items);
    $it@kept@ems;

    $text = "not an array";
    shuffled($text);
    $te@replaced@xt;
}
"#,
        &[
            Query::hover("a list the parameter admits", "kept"),
            Query::hover("a string it cannot", "replaced"),
        ],
        expect![[r#"
            a list the parameter admits
            - code: array<int, Item> $items
            - note: parameter
            a string it cannot
            - code: array $text
            - note: assigned
        "#]],
    );
}

/// A sentinel the caller wrote is not precision to keep.
///
/// `$out = null;` above `fills(?string &$out)` is the variable being declared, and the call is what
/// puts a value in it -- so keeping the null says the call left one, which is stronger than
/// anything either side wrote. It reported as an `error` on the ordinary shape of a populator: a
/// validator or a parser that takes `&$out` and writes into it. The declaration is the honest
/// weaker answer, and what it keeps is the null the caller may still be holding.
#[test]
fn a_null_the_caller_wrote_is_replaced_by_what_the_parameter_declares() {
    check(
        r#"
//- /src/app.php
<?php

function fills(?string &$out): void
{
    $out = "filled";
}

function run(): void
{
    $text = null;
    fills($text);
    $te@filled@xt;

    $flag = false;
    fills($flag);
    $fl@sentinel@ag;
}
"#,
        &[
            Query::hover("what the call may have left", "filled"),
            Query::hover("a false sentinel reads the same way", "sentinel"),
        ],
        expect![[r#"
            what the call may have left
            - code: ?string $text
            - note: assigned
            a false sentinel reads the same way
            - code: ?string $flag
            - note: assigned
        "#]],
    );
}
