//! `@return static<TKey, TMapValue>` beside `@param callable(TValue, TKey): TMapValue`.
//!
//! Laravel's `Collection::map()`. The method's own template is what the callback returns, and the
//! rest is the receiver: `static` is the collection it was called on and `TKey` what that one was
//! written with. Read without the call's scope, `static` named nothing -- every `->map(...)` ended
//! the chain it was in -- and a class written out took its key from the template's bound.
//!
//! Eloquent's override writes `Coll<TKey, TMapValue>|static<TKey, TMapValue>`: a collection of
//! models only where the callback made models, because `TModel of Model` is the subclass's bound.
//! A callback building arrays leaves the base collection alone.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_class_a_callback_fills_keeps_the_receiver() {
    check_view(
        r#"
//- /src/Coll.php
<?php
namespace App;

class User {}
class Channel {}

/**
 * @template TKey of array-key
 * @template TValue
 * @implements \IteratorAggregate<TKey, TValue>
 */
class Coll implements \IteratorAggregate
{
    /**
     * @template TMapValue
     * @param callable(TValue, TKey): TMapValue $callback
     * @return static<TKey, TMapValue>
     */
    public function map(callable $callback) {}

    /**
     * @template TMapValue
     * @param callable(TValue, TKey): TMapValue $callback
     * @return Coll<TKey, TMapValue>
     */
    public function mapNamed(callable $callback) {}

    /**
     * @param callable(TValue, TKey): mixed $callback
     * @return static<TKey, TValue>
     */
    public function filter(callable $callback) {}

}

/**
 * @template TKey of array-key
 * @template TModel of Model
 * @extends Coll<TKey, TModel>
 */
class Models extends Coll
{
    /**
     * @template TMapValue
     * @param callable(TModel, TKey): TMapValue $callback
     * @return Coll<TKey, TMapValue>|static<TKey, TMapValue>
     */
    public function map(callable $callback) {}
}

class Model {}

//- /src/run.php
<?php
namespace App;

/**
 * @param Coll<int, User> $users
 * @param Models<int, Model> $models
 */
function run(Coll $users, Models $models): void
{
    $rows = $models->map(fn (Model $m) => ['id' => 1]);
    $r@rows@ows;
    $kept = $models->map(fn (Model $m) => $m);
    $k@kept@ept;
    $mapped = $users->map(fn (User $u) => new Channel());
    $m@mapped@apped;
    $named = $users->mapNamed(fn (User $u) => new Channel());
    $n@named@amed;
    $filtered = $users->filter(fn (User $u) => true);
    $f@filtered@iltered;
}
"#,
        &[
            Query::variable_type("static, with a slot the callback fills", "mapped"),
            Query::variable_type("the class named, the receiver's key kept", "named"),
            Query::variable_type("static with no slot a call fills", "filtered"),
            Query::variable_type("an override returning either, given arrays", "rows"),
            Query::variable_type("and given models", "kept"),
        ],
        expect![[r#"
            static, with a slot the callback fills
            - Coll<int, Channel>
            the class named, the receiver's key kept
            - Coll<int, Channel>
            static with no slot a call fills
            - Coll<int, User>
            an override returning either, given arrays
            - Coll<int, array{id: int}>
            and given models
            - Coll<int, Model>|Models<int, Model>
        "#]],
    );
}
