//! What `array_map` and its neighbours are handed when the callback is not a closure literal.
//!
//! `$this->convert(...)`, `[$this, 'convert']`, `self::convert(...)` and a variable holding one all
//! resolve to a callable carrying its return -- the same answer `$callback()` reads -- and only
//! the literal was read, so `array_map($this->convert(...), $rows)` was a list of nothing. A string
//! names a function too, and is read only where it names one of the runtime's: a project's function
//! declares its return for every argument at once, and WordPress's `esc_sql` says `string|array`.
//!
//! Beside it, two readers the typed callback exposed. A reduce's first parameter is its
//! accumulator, which the container reading used to hand an element; and a spread argument passes
//! what the value holds, so `array_merge(...$groups)` merges the groups.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /stubs/standard.php  stub
<?php
/**
 * @template K as array-key
 * @template V
 * @template S
 * @template U
 *
 * @param (callable(V): U)|(callable(V, S): U)|null $callback
 * @param array<K, V> $array
 * @param array<S> ...$arrays
 *
 * @return ($array is list<V>
 *     ? ($array is non-empty-list<V> ? non-empty-list<U> : list<U>)
 *     : ($array is non-empty-array<K, V> ? non-empty-array<K, U> : array<K, U>))
 */
function array_map(?callable $callback, array $array, array ...$arrays): array {}

/**
 * @template V
 * @template I
 * @param array<V> $array
 * @param callable(I, V): I $callback
 * @param I $initial
 * @return I
 */
function array_reduce(array $array, callable $callback, mixed $initial = null): mixed {}

/**
 * @template K of array-key
 * @template V
 * @param array<K, V> ...$arrays
 * @return array<K, V>
 */
function array_merge(array ...$arrays): array {}

/**
 * @template T
 *
 * @param array<T> $array
 * @param (callable(T, T): int) $callback
 */
function usort(array &$array, callable $callback): true {}

function strlen(string $string): int {}

//- /src/Rows.php
<?php
namespace App;

class Item {}

function project_item(int $i): Item {}

class Rows
{
    public function convert(int $i): Item {}

    /**
     * @param list<int> $ids
     * @param list<list<string>> $groups
     * @param list<string> $names
     */
    public function run(array $ids, array $groups, array $names): void
    {
        foreach (array_map(fn (int $i): Item => new Item(), $ids) as $l@literal@iteral) {
        }
        foreach (array_map($this->convert(...), $ids) as $f@first_class@irst) {
        }
        foreach (array_map([$this, 'convert'], $ids) as $p@pair@air) {
        }
        foreach (array_map('strlen', $names) as $r@runtime@untime) {
        }
        foreach (array_map('App\project_item', $ids) as $o@own@wn) {
        }
        array_reduce($names, fn (array $c@carry@arry, string $name) => $carry, []);
        usort($names, fn ($left, $right) => $l@left@eft <=> $right);
        foreach (array_merge(...$groups) as $m@merged@erged) {
        }
    }
}
"#;

#[test]
fn a_callback_is_read_by_what_invoking_it_gives() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("a closure literal", "literal"),
            Query::variable_type("a first-class callable", "first_class"),
            Query::variable_type("`[$this, 'method']`", "pair"),
            Query::variable_type("a runtime function named by a string", "runtime"),
            Query::variable_type("a project function named by a string", "own"),
            Query::variable_type("a reduce's accumulator keeps its own hint", "carry"),
            Query::variable_type("a comparator is handed elements", "left"),
            Query::variable_type("what a spread merges", "merged"),
        ],
        expect![[r#"
            a closure literal
            - Item
            a first-class callable
            - Item
            `[$this, 'method']`
            - Item
            a runtime function named by a string
            - int
            a project function named by a string
            - (no type)
            a reduce's accumulator keeps its own hint
            - array
            a comparator is handed elements
            - string
            what a spread merges
            - string
        "#]],
    );
}
