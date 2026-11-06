//! A string that names a class, read as one: `Foo::class`, `$item::class`, `get_class($item)`.
//!
//! The card said `string`, which is true and leaves out the one thing the value is for, and `new`
//! over the last two made nothing -- `get_class($m)` was a call returning a string, and which class
//! it names was nobody's question. One reading answers both, so the card and the instantiation
//! cannot disagree. `$this` is refused as `static::class` is: it names whichever class the call was
//! made on, which a subclass inheriting the code makes something other than the enclosing one.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn each_spelling_names_the_class_and_new_makes_one() {
    check_view(
        r#"
//- /stubs/core.php  stub
<?php
function get_class(object $object): string {}

//- /src/app.php
<?php
namespace App;

class Item {}

class Maker
{
    public function run(Item $item): void
    {
        $w@written@ritten = Item::class;
        $i@instance@nstance = $item::class;
        $g@called@alled = get_class($item);
        $m@made@ade = new (get_class($item))();
        $o@own@wn = $this::class;
        $p@plain@lain = 'Item';
    }
}
"#,
        &[
            Query::variable_type("written out", "written"),
            Query::variable_type("read off an instance", "instance"),
            Query::variable_type("asked of get_class()", "called"),
            Query::variable_type("instantiated from get_class()", "made"),
            Query::variable_type("$this names whichever class the call was made on", "own"),
            Query::variable_type("a string is only a string", "plain"),
        ],
        expect![[r#"
            written out
            - class-string<Item>
            read off an instance
            - class-string<Item>
            asked of get_class()
            - class-string<Item>
            instantiated from get_class()
            - Item
            $this names whichever class the call was made on
            - non-falsy-string
            a string is only a string
            - non-falsy-string
        "#]],
    );
}
