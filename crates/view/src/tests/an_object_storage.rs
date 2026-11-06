//! A container whose loop and subscript give different things: `SplObjectStorage<O, V>`.
//!
//! Iterating it yields the objects and keys them by `int`; subscripting it runs `offsetGet` and
//! gives what each object maps to. The convention of taking a container's last argument answered
//! `V` for the loop and named every key after `O`, and a subscript whose key was an expression was
//! read as the loop. The class here carries the prelude's own tags.

use expect_test::expect;

use super::iterated_value::check_iterated;

#[test]
fn the_loop_reads_the_slot_the_tags_name_and_the_subscript_reads_offset_get() {
    check_iterated(
        r#"
//- /stubs/access.php
<?php
/**
 * @template K
 * @template V
 */
interface ArrayAccess
{
    /** @return V */
    public function offsetGet(mixed $offset): mixed;
}

//- /src/Storage.php
<?php
namespace App;

/**
 * @template O of object
 * @template V
 *
 * @template-implements \Iterator<int, O>
 * @template-implements \ArrayAccess<O, V>
 */
class Storage implements \Iterator, \ArrayAccess
{
    /** @return O */
    public function current(): object {}
    public function key(): int {}
    public function next(): void {}
    public function valid(): bool {}
    public function rewind(): void {}
    /**
     * @param O $object
     * @return V
     */
    public function offsetGet(mixed $object): mixed {}
}

/**
 * @template T of object
 * @extends Storage<T, string>
 */
class Labels extends Storage {}

class Item {}
class Meta {}

class Use_
{
    /** @return Storage<Item, Meta> */
    public function stored(): Storage {}

    /**
     * @param Storage<Item, Meta> $written
     * @param Labels<Item> $labels
     */
    public function run(Storage $written, Labels $labels): void
    {
        foreach ($written as $k@key@ey => $v@value@alue) {}
        $m@mapped@apped = $written[new Item()];
        foreach ($this->stored() as $sk@storedkey@ey => $sv@storedvalue@alue) {}
        $sm@storedmapped@apped = $this->stored()[new Item()];
        foreach ($labels as $l@label@abel) {}
    }
}
"#,
        &[
            "key",
            "value",
            "mapped",
            "storedkey",
            "storedvalue",
            "storedmapped",
            "label",
        ],
        expect![[r#"
            key: int
            value: Item
            mapped: Meta
            storedkey: int
            storedvalue: Item
            storedmapped: Meta
            label: Item
        "#]],
    );
}
