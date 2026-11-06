//! `($callback is null ? A : B)` settled by what the argument *is*, not only by the keyword.
//!
//! Laravel's `tap($value, $callback = null)` is `($callback is null ? HigherOrderTapProxy : TValue)`,
//! and it came back untyped 61 times in Laravel's own source for two reasons stacked. The `TValue`
//! branch is a template `@param TValue $value` binds outright, which the reader of a `Bar<TValue>`
//! slot knew and the reader of a branch did not -- so the whole contract was refused, and even
//! `tap($x)` said nothing. And the test read only a written `null`, so a closure settled nothing. A
//! closure is not null, and neither is any value whose type has no null in it; one that admits null,
//! or one nothing is known about, still settles nothing.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_value_whose_type_has_no_null_fails_the_test() {
    check(
        r#"
//- /src/helpers.php
<?php

class HigherOrderTapProxy {}
class Repository {}

/**
 * @template TValue
 * @param  TValue  $value
 * @param  (callable(TValue): mixed)|null  $callback
 * @return ($callback is null ? HigherOrderTapProxy : TValue)
 */
function tap($value, $callback = null) {}

//- /src/app.php
<?php

function run(Repository $repository, ?Closure $maybe, $unknown): void
{
    $closure = tap($repository, function ($r) {});
    $clo@closure@sure;
    $none = tap($repository);
    $no@none@ne;
    $keyword = tap($repository, null);
    $key@keyword@word;
    $nullable = tap($repository, $maybe);
    $nulla@nullable@ble;
    $untyped = tap($repository, $unknown);
    $unty@untyped@ped;
}
"#,
        &[
            Query::hover("a closure", "closure"),
            Query::hover("no callback", "none"),
            Query::hover("null written", "keyword"),
            Query::hover("a value that may be null", "nullable"),
            Query::hover("a value nothing is known about", "untyped"),
        ],
        expect![[r#"
            a closure
            - code: Repository $closure
            - note: assigned
            no callback
            - code: HigherOrderTapProxy $none
            - note: assigned
            null written
            - code: HigherOrderTapProxy $keyword
            - note: assigned
            a value that may be null
            - code: HigherOrderTapProxy|Repository $nullable
            - note: assigned
            a value nothing is known about
            - code: HigherOrderTapProxy|Repository $untyped
            - note: assigned
        "#]],
    );
}

/// A branch that is nothing but `null` is the absence of the other, and a guard or a `??` takes it
/// off. Joined as a stored operand, the null stood where neither could reach it: `array_shift()`'s
/// `($array is non-empty-array ? V : null)` under `if (null !== $taken)` read as possibly null.
#[test]
fn a_branch_that_is_only_null_is_taken_off_by_a_guard() {
    crate::tests::diagnostics::check(
        r#"
//- /src/functions.php
<?php
/**
 * @template V
 * @param array<V> $array
 * @return ($array is non-empty-array ? V : null)
 */
function shift(array &$array): mixed {}

/**
 * @template K of array-key
 * @param array<K, mixed> $array
 * @return ($array is non-empty-array ? K : null)
 */
function first_key(array $array): int|string|null {}

//- /src/Queue.php
<?php
class Suspension
{
    public function resume(): void {}
}

class Queue
{
    /** @var list<Suspension> */
    private array $pending = [];

    /** @var array<int, string> */
    private array $terms = [];

    public function next(): void
    {
        $taken = shift($this->pending);
        if (null !== $taken) {
            $taken->resume();
        }
    }

    public function first(): int
    {
        return first_key($this->terms) ?? 0;
    }
}
"#,
        expect![[r#""#]],
    );
}
