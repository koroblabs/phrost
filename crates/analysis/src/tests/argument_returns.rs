//! A function whose return type is spelled the same as one of its parameters.
//!
//! `array_filter` and its whole family say only one thing: whatever you passed is what you get back.
//! The declaration cannot name the type — it names the argument to read it from — and that is the
//! plainest generic there is, which is why the same contract carries it for a method and for a free
//! function.
//!
//! Everything here would otherwise resolve to a class called `V`, or to nothing at all.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_return_spelled_like_a_parameter_is_the_argument() {
    check(
        r#"
//- /app.php
<?php
class Row
{
    public function id(): int {}
}

/**
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @return array<K, V>
 */
function keep(array $array, callable $callback): array {}

/** @return array<int, Row> */
function loadAll(): array {}

function run(): void
{
    $kept = keep(loadAll(), fn (Row $row): bool => TRUE);
    $@here@kept;
    foreach ($kept as $one) {
        $one->$0
    }
}
"#,
        &[
            Query::hover("the container it was given", "here"),
            Query::completion("an element of it", "0"),
        ],
        expect![[r#"
            the container it was given
            - code: array<int, Row> $kept
            - note: assigned
            an element of it
            - id [method]  : int  writes `();`
            - accepting the first leaves `$one->id`
        "#]],
    );
}

#[test]
fn a_null_only_the_parameter_carries_is_looked_through() {
    // `array_reduce` takes `null|I $initial` and returns `I`. The null is the parameter's own
    // default and not something the value carries out, so it must not stop the two matching.
    check(
        r#"
//- /app.php
<?php
class Total
{
    public function amount(): int {}
}

/**
 * @template V
 * @template I
 *
 * @param array<V> $array
 * @param (callable(I, V): I) $callback
 * @param null|I $initial
 *
 * @return I
 */
function fold(array $array, callable $callback, mixed $initial = NULL): mixed {}

function run(array $rows): void
{
    $total = fold($rows, fn ($carry, $row) => $carry, new Total());
    $total->$0
}
"#,
        &[Query::completion("what the initial value was", "0")],
        expect![[r#"
            what the initial value was
            - amount [method]  : int  writes `();`
            - accepting the first leaves `$total->amount`
        "#]],
    );
}

#[test]
fn a_container_of_what_the_argument_holds() {
    // `array_values` and `array_reverse` write the same claim four ways -- `list<V>`,
    // `non-empty-list<V>`, `array<K, V>` -- and what the branches disagree about is the keying.
    check(
        r#"
//- /app.php
<?php
class Row
{
    public function id(): int {}
}

/**
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @return ($array is non-empty-array ? non-empty-list<V> : list<V>)
 */
function values(array $array): array {}

/** @return array<string, Row> */
function loadAll(): array {}

function run(): void
{
    $rows = values(loadAll());
    $@here@rows;
    foreach ($rows as $one) {
        $one->$0
    }
}
"#,
        &[
            Query::hover("a container of the elements", "here"),
            Query::completion("one of them", "0"),
        ],
        expect![[r#"
            a container of the elements
            - code: Row[] $rows
            - note: assigned
            one of them
            - id [method]  : int  writes `();`
            - accepting the first leaves `$one->id`
        "#]],
    );
}

#[test]
fn an_element_bound_through_the_null_the_parameter_carries() {
    // `@param array<K, V|null>` beside `@return V` is how a helper that drops the empties is
    // written, and the null is the *parameter's* -- it says nothing about what comes back. The
    // rule for looking through one already existed for a parameter's own type (`array_reduce`
    // takes `null|I $initial` and returns `I`); the element position is where it was missing, so
    // `V` bound from `array<K, V>` and answered nothing from `array<K, V|null>`.
    check(
        r#"
//- /app.php
<?php
class Row
{
    public function id(): int {}
}

/**
 * @template K as array-key
 * @template V
 *
 * @param array<K, V|null> $rows
 *
 * @return V
 */
function firstPresent(array $rows) {}

/** @return array<string, Row> */
function loadAll(): array {}

function run(): void
{
    $row = firstPresent(loadAll());
    $@here@row;
    $row->$0
}
"#,
        &[
            Query::hover("the element, without the parameter's null", "here"),
            Query::completion("and its members", "0"),
        ],
        expect![[r#"
            the element, without the parameter's null
            - code: Row $row
            - note: assigned
            and its members
            - id [method]  : int  writes `();`
            - accepting the first leaves `$row->id`
        "#]],
    );
}

#[test]
fn a_container_of_what_the_callback_returns() {
    // `array_map` is the whole of PHP's answer to `map`, and the closure written at the call is the
    // only place its result type appears.
    check(
        r#"
//- /app.php
<?php
class Row
{
    public function id(): int {}
}
class Item
{
    public function label(): string {}
}

/**
 * @template K as array-key
 * @template V
 * @template U
 *
 * @param (callable(V): U)|null $callback
 * @param array<K, V> $array
 *
 * @return ($array is list<V> ? list<U> : array<K, U>)
 */
function mapped(?callable $callback, array $array): array {}

/** @return list<Row> */
function loadAll(): array {}

function run(): void
{
    $items = mapped(fn (Row $row): Item => new Item(), loadAll());
    $@here@items;
    foreach ($items as $one) {
        $one->$0
    }
}
"#,
        &[
            Query::hover("a container of the callback's results", "here"),
            Query::completion("one of them", "0"),
        ],
        expect![[r#"
            a container of the callback's results
            - code: Item[] $items
            - note: assigned
            one of them
            - label [method]  : string  writes `();`
            - accepting the first leaves `$one->label`
        "#]],
    );
}

#[test]
fn what_the_callback_returns_is_what_a_reduce_gives_back() {
    // Every `reduce` in every collection library. The declaration cannot name its result -- the
    // closure at the call is the only place that type is written.
    check(
        r#"
//- /app.php
<?php
class Decimal {
    public function add(Decimal $other): Decimal { return $this; }
    public function toFixed(int $precision): string { return ''; }
}

class Orderproduct {
    public Decimal $price;
}

/** @template TValue */
class ProductCollection {
    /**
     * @template TReduceInitial
     * @template TReduceReturnType
     *
     * @param callable(TReduceInitial|TReduceReturnType, TValue): TReduceReturnType $callback
     * @param TReduceInitial $initial
     * @return TReduceReturnType
     */
    public function reduce(callable $callback, $initial) {}
}

function run(ProductCollection $products): void
{
    $total = $products->reduce(
        fn (Decimal $carry, Orderproduct $p): Decimal => $carry->add($p->price),
        new Decimal()
    );
    $@here@total;
    $total->$0
}
"#,
        &[
            Query::hover("what reduce gave back", "here"),
            Query::completion("through it", "0"),
        ],
        expect![[r#"
            what reduce gave back
            - code: Decimal $total
            - note: assigned
            through it
            - add [method]  : Decimal  writes `(|);`
            - toFixed [method]  : string  writes `(|);`
            - accepting the first leaves `$total->add`
        "#]],
    );
}

#[test]
fn an_arrow_functions_result_is_read_off_its_body() {
    // An arrow function's body *is* one expression, already lowered, in the file being read. So a
    // callback written without a return type still says what it gives back — which is how most of
    // them are written.
    check(
        r#"
//- /app.php
<?php
class Decimal {
    public function add(Decimal $other): Decimal { return $this; }
    public function toFixed(int $precision): string { return ''; }
}

class Orderproduct {
    public function getPrice(): Decimal {}
}

/** @template TValue */
class ProductCollection {
    /**
     * @template TReduceInitial
     * @template TReduceReturnType
     *
     * @param callable(TReduceInitial|TReduceReturnType, TValue): TReduceReturnType $callback
     * @param TReduceInitial $initial
     * @return TReduceReturnType
     */
    public function reduce(callable $callback, $initial = NULL) {}
}

function run(ProductCollection $products): void
{
    $total = $products->reduce(fn (Decimal $carry, $op) => $carry->add($op->getPrice()), new Decimal());
    $@here@total;
}
"#,
        &[Query::hover("inferred from the arrow body", "here")],
        expect![[r#"
            inferred from the arrow body
            - code: Decimal $total
            - note: assigned
        "#]],
    );
}

#[test]
fn a_container_keyed_by_another_argument_is_keyed_by_what_that_argument_holds() {
    // `array_combine` is keyed by what its first argument holds and filled with what its second
    // holds. Reduced to "the elements of the second" it came back a list, so every key read off
    // the result was an `int` -- the one thing a table of names is not. This pinned the refusal
    // until a branch could say where the key comes from, and `CombinedElements` is that branch.
    check(
        r#"
//- /app.php
<?php
/**
 * @template K as array-key
 * @template V
 *
 * @param array<K> $keys
 * @param array<V> $values
 *
 * @return ($keys is non-empty-array ? non-empty-array<K, V> : array<K, V>)
 */
function combine(array $keys, array $values): array {}

/**
 * @param list<string> $names
 * @param list<int> $sizes
 */
function run(array $names, array $sizes): void
{
    foreach (combine($names, $sizes) as $@key@name => $size) {
    }
}
"#,
        &[Query::hover("a key of the combined table", "key")],
        expect![[r#"
            a key of the combined table
            - code: string $name
            - note: loop variable
        "#]],
    );
}

#[test]
fn a_plain_container_return_is_bound_like_the_conditional_one() {
    // `array_map` writes its return as a four-branch conditional and a project writes the same
    // claim once -- `@return list<U>` beside `@param callable(V): U` -- and only the first bound
    // anything. A class with arguments is not this: `Collection<TKey, TValue>` is what the
    // arguments belong to, and read as a list every Laravel `collect()` came back an array.
    check(
        r#"
//- /app.php
<?php
class Row
{
    public function id(): int {}
}

/**
 * @template V
 * @template U
 *
 * @param list<V> $items
 * @param callable(V): U $map
 *
 * @return list<U>
 */
function map_all(array $items, callable $map): array {}

function run(): void
{
    foreach (map_all([1, 2], fn (int $i): Row => new Row()) as $row) {
        $row->$0
    }
}
"#,
        &[Query::completion("what the callback returned", "0")],
        expect![[r#"
            what the callback returned
            - id [method]  : int  writes `();`
            - accepting the first leaves `$row->id`
        "#]],
    );
}
