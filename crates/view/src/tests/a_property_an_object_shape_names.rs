//! A property an `object{...}` shape names, read off the shape in each spelling it arrives in.
//!
//! The shape is no class, so no member table has the property: written in this file, interned from
//! another file's declaration, behind a `?`, and beside a class in an intersection. A property the
//! shape does not name is not answered by it.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_property_is_read_off_the_shape_that_names_it() {
    check_view(
        r#"
//- /src/Cache.php
<?php
namespace App;

class Row {}

class Cache
{
    /** @return object{cid: string, data: Row}|null */
    public function get(string $cid) {}

    /** @return Row&object{extra: int} */
    public function decorated() {}
}

//- /src/run.php
<?php
namespace App;

function run(Cache $cache): void
{
    /** @var object{cid: string, data: Row} $written */
    $written = load();
    $w@written@ritten = $written->data;

    $item = $cache->get('x');
    $s@stored@tored = $item->data;
    $c@cid@id = $item?->cid;

    $d@extra@xtra = $cache->decorated()->extra;
    $n@unnamed@nnamed = $item->missing;
}
"#,
        &[
            Query::variable_type("written in this file", "written"),
            Query::variable_type("from another file's declaration, behind a ?", "stored"),
            Query::variable_type("through ?->", "cid"),
            Query::variable_type("beside a class", "extra"),
            Query::variable_type("a property the shape does not name", "unnamed"),
        ],
        expect![[r#"
            written in this file
            - Row
            from another file's declaration, behind a ?
            - Row
            through ?->
            - ?string
            beside a class
            - int
            a property the shape does not name
            - (no type)
        "#]],
    );
}
