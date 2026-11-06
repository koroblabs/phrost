use super::{cache::check_round_trip, check_index};
use expect_test::expect;

#[test]
fn probe() {
    check_index(
        r#"
//- /src/Models.php
<?php
namespace App;

/** @template TKey @template TModel */
class Models
{
    /**
     * @template TMapValue
     * @param callable(TModel, TKey): TMapValue $callback
     * @return Coll<TKey, TMapValue>|static<TKey, TMapValue>
     */
    public function map(callable $callback) {}

    /**
     * @return Coll<TKey, TModel>|static<TKey, TModel>
     */
    public function plain() {}
}
"#,
        expect![[r#"
            state ready
            file src/Models.php [User]
              class App\Models @ 5:7-5:13
              App\Models is class
              App\Models template TKey
              App\Models::method map(callable $callback)  public return names its own template $callback calls(\App\TModel, \App\TKey) returns \App\Coll or static of \App\TKey, what the callable at 0 returns  @ 12:21-12:24 (full 12:5-12:47)
              App\Models::method plain(): Coll<TKey, TModel>|static<TKey, TModel>  public  @ 17:21-17:26 (full 17:5-17:31)
            catalog
              [class] app\models -> src/Models.php (App\Models)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

/// The alternatives a union of one argument list writes beside its class, read back from disk.
#[test]
fn the_alternatives_of_an_applied_return_survive_the_cache() {
    check_round_trip(
        r#"
//- /src/Models.php
<?php
namespace App;

/** @template TKey @template TModel */
class Models
{
    /**
     * @template TMapValue
     * @param callable(TModel, TKey): TMapValue $callback
     * @return Coll<TKey, TMapValue>|static<TKey, TMapValue>
     */
    public function map(callable $callback) {}
}
"#,
    );
}
