//! A closure handed to `Closure::bind` runs as the class it was bound to.
//!
//! It is how PHP reaches into another object's internals on purpose, and Symfony's Cache component
//! is built on it: every adapter makes its `CacheItem`s through a closure bound to `CacheItem::class`,
//! because the item's fields are protected and the adapter is not the item. Read as the class the
//! closure is *written* in, those accesses are 55 findings on code PHP runs.
//!
//! The scope has to be written as a class for this to see it. `Closure::bind($fn, $object)` takes the
//! scope from `$object`'s class, and `$this::class` says one dynamically -- both leave the answer to
//! the enclosing class, which is what the code was already read as.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn the_bound_scope_is_what_reaches_a_protected_member() {
    check(
        r#"
//- /stubs/closure.php  stub
<?php
final class Closure
{
    public static function bind(\Closure $closure, ?object $newThis, object|string|null $newScope = 'static'): ?\Closure {}

    public function bindTo(?object $newThis, object|string|null $newScope = 'static'): ?\Closure {}
}

//- /src/Bound.php
<?php
namespace App;

class Item
{
    protected string $key = '';

    protected function touch(): void {}
}

class Maker
{
    public static function make(): ?\Closure
    {
        return \Closure::bind(
            static function (string $key): Item {
                $item = new Item();
                $item->key = $key;
                $item->touch();

                return $item;
            },
            null,
            Item::class
        );
    }

    public function outside(): void
    {
        $item = new Item();
        $item->key = 'x';
    }
}
"#,
        expect![[r#"
            31:16-31:19 error [inaccessible-member] Cannot access protected property App\Item::$key
        "#]],
    );
}
