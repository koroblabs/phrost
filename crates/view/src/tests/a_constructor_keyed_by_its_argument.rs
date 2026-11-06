//! `new ArrayIterator([1, 2, 3])` -- a class template a constructor binds from its argument's *keys*.
//!
//! `@param array<K, V> $array` binds two templates, and only the element was read: `V` came from what
//! the argument holds and `K` fell to its bound, `array-key`, so `key()` answered `int|string` for a
//! list. What a key is follows from how the argument was written, which is the question a `foreach`
//! over it asks.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_key_template_is_what_the_argument_is_keyed_by() {
    check_view(
        r#"
//- /src/Bag.php
<?php
namespace App;

class Row {}

/**
 * @template K of array-key
 * @template V
 */
class Bag
{
    /** @param array<K, V> $array */
    public function __construct(array $array = []) {}

    /** @return K|null */
    public function key() {}
}

class Caller
{
    /** @param array<string, Row> $byName */
    public function run(array $byName, array $bare): void
    {
        $listed = new Bag([new Row(), new Row()]);
        $l@listed@isted;
        $named = new Bag(['a' => new Row()]);
        $n@named@amed;
        $typed = new Bag($byName);
        $t@typed@yped;
        $key = $listed->key();
        $k@key@ey;
        $untyped = new Bag($bare);
        $u@untyped@ntyped;
    }
}
"#,
        &[
            Query::variable_type("a list literal is keyed by int", "listed"),
            Query::variable_type("a keyed literal by its keys", "named"),
            Query::variable_type("a typed argument by its declared key", "typed"),
            Query::variable_type("what key() gives back", "key"),
            Query::variable_type("an argument that says nothing binds nothing", "untyped"),
        ],
        expect![[r#"
            a list literal is keyed by int
            - Bag<int, Row>
            a keyed literal by its keys
            - Bag<non-falsy-string, Row>
            a typed argument by its declared key
            - Bag<string, Row>
            what key() gives back
            - ?int
            an argument that says nothing binds nothing
            - Bag
        "#]],
    );
}
