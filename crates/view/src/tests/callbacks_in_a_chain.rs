//! What a closure passed along a fluent chain receives.
//!
//! Three rules that each hid behind the others: every link of one chain starts at the same offset,
//! so a remembered declaration has to say *which* link it is; a callable that is one operand of a
//! union is still the only thing a closure literal can be; and a `static` in a callback's signature
//! is the object the call was made on.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn each_link_of_a_chain_hands_its_own_callback_what_it_declares() {
    check_view(
        r#"
//- /src/Coll.php
<?php
namespace App;

class Item { public function label(): string { return ''; } }

/** @template TValue */
class Coll
{
    /** @param list<TValue> $items */
    public function __construct(array $items = []) {}

    /**
     * @param callable(Item): mixed $callback
     * @return static
     */
    public function map(callable $callback) { return $this; }

    /**
     * @param (callable($this): mixed)|null $callback
     * @return $this
     */
    public function when(bool $value, ?callable $callback = null) { return $this; }

    /**
     * @param (\Closure(static): mixed)|string|array $column
     * @return $this
     */
    public function where($column) { return $this; }
}

function run(Item $item): void
{
    (new Coll([$item]))
        ->map(fn ($v@mapped@alue) => $value)
        ->when(true, fn ($wh@when@en) => $when)
        ->where(fn ($qu@where@ery) => $query);
}
"#,
        &[
            Query::variable_type("the first link's callback, not the last one's", "mapped"),
            Query::variable_type("a callable that is one operand of a nullable union", "when"),
            Query::variable_type(
                "and one beside a string and an array, written static",
                "where",
            ),
        ],
        expect![[r#"
            the first link's callback, not the last one's
            - Item
            a callable that is one operand of a nullable union
            - Coll<Item>
            and one beside a string and an array, written static
            - Coll<Item>
        "#]],
    );
}
