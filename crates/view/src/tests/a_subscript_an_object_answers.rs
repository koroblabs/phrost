//! `$object[$key]`, which PHP runs as `offsetGet($key)`.
//!
//! So the subscript is whatever `offsetGet` declares, when no `@implements ArrayAccess<K, V>` said
//! it first. Only for a subscript: iterating the same object runs `getIterator`, and WordPress's
//! cookie `Jar` answers `string|null` from one and hands out `Cookie` objects from the other.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_subscript_is_what_offset_get_declares_and_an_iteration_is_not() {
    check_view(
        r#"
//- /src/Jar.php
<?php
namespace App;

class Cookie {}

class Jar implements \ArrayAccess, \IteratorAggregate
{
    public function offsetExists(mixed $offset): bool {}
    public function offsetGet(mixed $offset): Cookie {}
    public function offsetSet(mixed $offset, mixed $value): void {}
    public function offsetUnset(mixed $offset): void {}
    public function getIterator(): \Iterator {}
}

class Runner
{
    public function go(Jar $jar, string $name): void
    {
        $l@literal@iteral = $jar['session'];
        $v@variable@ariable = $jar[$name];
        foreach ($jar as $e@iterated@ach) {
        }
    }
}
"#,
        &[
            Query::variable_type("a literal key", "literal"),
            Query::variable_type("a variable key", "variable"),
            Query::variable_type("iterating it", "iterated"),
        ],
        expect![[r#"
            a literal key
            - Cookie
            a variable key
            - Cookie
            iterating it
            - (no type)
        "#]],
    );
}
