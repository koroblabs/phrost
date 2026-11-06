//! `@phpstan-self-out` and its spellings -- what a method leaves in its receiver -- stored whole.
//!
//! Held here as well as at the view, for the reason `applied_contracts` gives: the class is an
//! interned type and each slot a branch, and a codec or renderer dropping either is invisible to a
//! feature test on a freshly built index. The dump is what both index invariants compare.

use expect_test::expect;

use super::{Step, check_incremental, check_index};

const BOX: &str = r#"
//- /src/Box.php
<?php
namespace App;

/** @template T */
final class Box
{
    /**
     * @template U
     * @param U $value
     * @phpstan-self-out self<U>
     */
    public function replace(mixed $value): void {}
}
"#;

#[test]
fn a_self_out_bound_by_an_argument_is_in_the_dump_and_survives_the_cache() {
    check_index(
        BOX,
        expect![[r#"
            state ready
            file src/Box.php [User]
              class App\Box @ 5:13-5:16
              App\Box is class final
              App\Box template T
              App\Box::method replace(mixed $value): void  public leaves its receiver self of whatever argument 0 is  @ 12:21-12:28 (full 12:5-12:51)
            catalog
              [class] app\box -> src/Box.php (App\Box)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
    super::cache::check_round_trip(BOX);
}

/// The Laravel spelling: `static`, a slot the receiver supplies, and one a callback fills.
#[test]
fn a_this_out_naming_the_receivers_own_template_is_stored_as_written() {
    check_index(
        r#"
//- /src/Items.php
<?php
namespace App;

/**
 * @template TKey of array-key
 * @template TValue
 */
class Items
{
    /**
     * @template TMapValue
     * @param callable(TValue, TKey): TMapValue $callback
     * @return $this
     * @phpstan-this-out static<TKey, TMapValue>
     */
    public function transform(callable $callback) { return $this; }
}
"#,
        expect![[r#"
            state ready
            file src/Items.php [User]
              class App\Items @ 8:7-8:12
              App\Items is class
              App\Items template TKey of int|string
              App\Items template TValue
              App\Items::method transform(callable $callback): static  public $callback calls(\App\TValue, \App\TKey) leaves its receiver static of \App\TKey, what the callable at 0 returns  @ 16:21-16:30 (full 16:5-16:68)
            catalog
              [class] app\items -> src/Items.php (App\Items)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

/// Symfony's `setParent()`: a slot naming the method's own template with nothing tying it to an
/// argument stores nothing, rather than a class nobody declares.
#[test]
fn a_self_out_whose_template_nothing_binds_is_not_stored() {
    check_index(
        r#"
//- /src/Node.php
<?php
namespace App;

/** @template TParent */
class Node
{
    /**
     * @template TNewParent
     * @psalm-this-out static<TNewParent>
     * @return $this
     */
    public function setParent(object $parent): static { return $this; }
}
"#,
        expect![[r#"
            state ready
            file src/Node.php [User]
              class App\Node @ 5:7-5:11
              App\Node is class
              App\Node template TParent
              App\Node::method setParent(object $parent): static  public  @ 12:21-12:30 (full 12:5-12:72)
            catalog
              [class] app\node -> src/Node.php (App\Node)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_self_out_survives_an_edit() {
    check_incremental(
        BOX,
        &[Step::write(
            "src/Box.php",
            r#"<?php
namespace App;

/** @template T */
final class Box
{
    /**
     * @template U
     * @param U $value
     * @phpstan-self-out self<U>
     */
    public function replace(mixed $value): void {}

    /** @phpstan-self-out self<int> */
    public function reset(): void {}
}
"#,
        )],
        expect![[r#"
            state ready
            file src/Box.php [User]
              class App\Box @ 5:13-5:16
              App\Box is class final
              App\Box template T
              App\Box::method replace(mixed $value): void  public leaves its receiver self of whatever argument 0 is  @ 12:21-12:28 (full 12:5-12:51)
              App\Box::method reset(): void  public leaves its receiver self of int  @ 15:21-15:26 (full 15:5-15:37)
            catalog
              [class] app\box -> src/Box.php (App\Box)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}
