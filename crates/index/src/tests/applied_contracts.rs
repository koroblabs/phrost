//! `@return Bar<T>` -- a class the declaration names, with arguments only the call fills.
//!
//! Held here rather than only at the view, because the two halves of the shape are stored in
//! different places: the class is an interned type and the arguments are branches, and a codec or a
//! renderer that drops either is invisible to a feature test on a freshly built index. The dump is
//! what both index invariants -- `cold == warm` and `incremental == from-scratch` -- compare, so a
//! contract that is not rendered is a contract neither of them can see.

use expect_test::expect;

use super::{Step, check_incremental, check_index};

#[test]
fn a_return_of_a_class_with_a_class_string_argument_is_stored_whole() {
    check_index(
        r#"
//- /src/Repo.php
<?php
namespace App;

/** @template T */
class Bag {}

class Factory
{
    /**
     * @template T
     * @param class-string<T> $class
     * @return Bag<T>
     */
    public function bagOf(string $class) {}
}
"#,
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Bag @ 5:7-5:10
              class App\Factory @ 7:7-7:14
              App\Bag is class
              App\Bag template T
              App\Factory is class
              App\Factory::method bagOf(string $class)  public return names its own template returns \App\Bag of the class argument 0 names  @ 14:21-14:26 (full 14:5-14:44)
            catalog
              [class] app\bag -> src/Repo.php (App\Bag)
              [class] app\factory -> src/Repo.php (App\Factory)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn every_kind_of_argument_a_slot_can_hold_survives_an_edit() {
    // Three slots, three sources: one the call names outright, one the declaration wrote down, and
    // one that is whatever was passed. A slot read the wrong way round would render the wrong source
    // here rather than merely resolving to nothing.
    check_incremental(
        r#"
//- /src/Repo.php
<?php
namespace App;

/**
 * @template A
 * @template B
 * @template C
 */
class Triple {}

class Marker {}

class Factory
{
    /**
     * @template T
     * @template U
     * @param class-string<T> $class
     * @param U $seed
     * @return Triple<T, Marker, U>
     */
    public function tripleOf(string $class, $seed) {}
}
"#,
        &[Step::write(
            "src/Repo.php",
            r#"<?php
namespace App;

/**
 * @template A
 * @template B
 * @template C
 */
class Triple {}

class Marker {}

class Factory
{
    /**
     * @template T
     * @template U
     * @param class-string<T> $class
     * @param U $seed
     * @return Triple<T, Marker, U>
     */
    public function tripleOf(string $class, $seed) {}

    public function plain(): Marker {}
}
"#,
        )],
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Triple @ 9:7-9:13
              class App\Marker @ 11:7-11:13
              class App\Factory @ 13:7-13:14
              App\Triple is class
              App\Triple template A
              App\Triple template B
              App\Triple template C
              App\Marker is class
              App\Factory is class
              App\Factory::method tripleOf(string $class, $seed)  public return names its own template returns \App\Triple of the class argument 0 names, \App\Marker, whatever argument 1 is  @ 22:21-22:29 (full 22:5-22:54)
              App\Factory::method plain(): Marker  public  @ 24:21-24:26 (full 24:5-24:39)
            catalog
              [class] app\factory -> src/Repo.php (App\Factory)
              [class] app\marker -> src/Repo.php (App\Marker)
              [class] app\triple -> src/Repo.php (App\Triple)
            namespaces
              app -> App x3
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_class_whose_arguments_are_all_written_down_is_a_plain_return_type() {
    // `Bag<Marker>` names two real classes and needs no call to settle it, so it stays what the
    // declaration already said. A contract there would be a second spelling of one answer.
    check_index(
        r#"
//- /src/Repo.php
<?php
namespace App;

/** @template T */
class Bag {}

class Marker {}

class Factory
{
    /**
     * @template T
     * @return Bag<Marker>
     */
    public function bagOfMarkers() {}
}
"#,
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Bag @ 5:7-5:10
              class App\Marker @ 7:7-7:13
              class App\Factory @ 9:7-9:14
              App\Bag is class
              App\Bag template T
              App\Marker is class
              App\Factory is class
              App\Factory::method bagOfMarkers(): Bag<Marker>  public  @ 15:21-15:33 (full 15:5-15:38)
            catalog
              [class] app\bag -> src/Repo.php (App\Bag)
              [class] app\factory -> src/Repo.php (App\Factory)
              [class] app\marker -> src/Repo.php (App\Marker)
            namespaces
              app -> App x3
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_list_of_such_a_class_is_stored_with_the_container_said() {
    check_index(
        r#"
//- /src/Repo.php
<?php
namespace App;

/** @template T */
class Bag {}

class Factory
{
    /**
     * @template T
     * @param class-string<T> $class
     * @return list<Bag<T>>
     */
    public function bagsOf(string $class): array {}
}
"#,
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Bag @ 5:7-5:10
              class App\Factory @ 7:7-7:14
              App\Bag is class
              App\Bag template T
              App\Factory is class
              App\Factory::method bagsOf(string $class): array  public return names its own template returns a list of \App\Bag of the class argument 0 names  @ 14:21-14:27 (full 14:5-14:52)
            catalog
              [class] app\bag -> src/Repo.php (App\Bag)
              [class] app\factory -> src/Repo.php (App\Factory)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_container_keyed_by_one_argument_and_holding_another_is_stored_whole() {
    // `array_combine`: the elements branch refuses a key the values argument does not own, so the
    // two parameters travel together and both have to survive the codec.
    check_index(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Combiner
{
    /**
     * @template K as array-key
     * @template V
     * @param array<K> $keys
     * @param array<V> $values
     * @return array<K, V>
     */
    public function combine(array $keys, array $values): array {}
}
"#,
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Combiner @ 4:7-4:15
              App\Combiner is class
              App\Combiner::method combine(array $keys, array $values): array  public return names its own template returns a container keyed by what argument 0 holds, of what argument 1 holds  @ 13:21-13:28 (full 13:5-13:66)
            catalog
              [class] app\combiner -> src/Repo.php (App\Combiner)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

/// A slot the parameter's container fills with its *keys*, beside one it fills with its elements:
/// Laravel's `collect()`. The slot read only elements, refused the key, and the whole contract with
/// it.
#[test]
fn a_slot_a_containers_keys_fill_is_stored() {
    check_index(
        r#"
//- /src/Repo.php
<?php
namespace App;

/** @template TKey @template TValue */
class Collection {}

/** @template TKey @template TValue */
interface Arrayable {}

class Helpers
{
    /**
     * @template TKey of array-key
     * @template TValue
     * @param Arrayable<TKey, TValue>|iterable<TKey, TValue>|null $value
     * @return Collection<TKey, TValue>
     */
    public function collect($value = []) {}
}
"#,
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Collection @ 5:7-5:17
              interface App\Arrayable @ 8:11-8:20
              class App\Helpers @ 10:7-10:14
              App\Collection is class
              App\Collection template TKey
              App\Arrayable is interface
              App\Arrayable template TKey
              App\Helpers is class
              App\Helpers::method collect($value = [])  public return names its own template returns \App\Collection of what names a slot of argument 0, what argument 0 holds  @ 18:21-18:28 (full 18:5-18:44)
            catalog
              [class] app\arrayable -> src/Repo.php (App\Arrayable)
              [class] app\collection -> src/Repo.php (App\Collection)
              [class] app\helpers -> src/Repo.php (App\Helpers)
            namespaces
              app -> App x3
            declared keys
            dispatched keys
        "#]],
    );
}

const GROUPED: &str = r#"
//- /src/Coll.php
<?php
namespace App;

/** @template TKey @template TValue */
class Coll
{
    /**
     * @template TGroupKey of array-key
     * @param (callable(TValue, TKey): TGroupKey)|array|string $groupBy
     * @param bool $preserveKeys
     * @return static<($groupBy is string ? array-key : TGroupKey), static<($preserveKeys is true ? TKey : int), TValue>>
     */
    public function groupBy($groupBy, $preserveKeys = false) {}
}
"#;

/// A slot that is a conditional, and one that is a class with arguments of its own: Laravel's
/// `groupBy()`. Both are stored as what they are, with the default the call leaves out.
#[test]
fn a_conditional_slot_and_a_nested_class_are_stored() {
    check_index(
        GROUPED,
        expect![[r#"
            state ready
            file src/Coll.php [User]
              class App\Coll @ 5:7-5:11
              App\Coll is class
              App\Coll template TKey
              App\Coll::method groupBy($groupBy, bool $preserveKeys = false)  public return names its own template $groupBy calls(\App\TValue, \App\TKey) returns static of (if argument 0 is a string, int|string and otherwise what the callable at 0 returns), static of (if argument 1 is true, \App\TKey and otherwise int by default false), \App\TValue  @ 13:21-13:28 (full 13:5-13:64)
            catalog
              [class] app\coll -> src/Coll.php (App\Coll)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_conditional_slot_survives_the_cache() {
    super::cache::check_round_trip(GROUPED);
}
