//! `object{pivot: Pivot}` is stored as the shape it is, and read back from disk the same.
//!
//! Lowered to a bare `object` it lost the one thing it says, which is what each property holds --
//! and the property is how Laravel's `BelongsToMany` states what `->pivot` is.

use expect_test::expect;

use super::{cache::check_round_trip, check_index};

const FIXTURE: &str = r#"
//- /src/Rows.php
<?php
namespace App;

class Row {}
class Pivot {}

class Rows
{
    /** @return Row&object{pivot: Pivot, position?: int} */
    public function first() {}
}
"#;

#[test]
fn an_object_shape_keeps_its_properties() {
    check_index(
        FIXTURE,
        expect![[r#"
            state ready
            file src/Rows.php [User]
              class App\Row @ 4:7-4:10
              class App\Pivot @ 5:7-5:12
              class App\Rows @ 7:7-7:11
              App\Row is class
              App\Pivot is class
              App\Rows is class
              App\Rows::method first(): Row&object{pivot: Pivot, position?: int}  public  @ 10:21-10:26 (full 10:5-10:31)
            catalog
              [class] app\pivot -> src/Rows.php (App\Pivot)
              [class] app\row -> src/Rows.php (App\Row)
              [class] app\rows -> src/Rows.php (App\Rows)
            namespaces
              app -> App x3
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn an_object_shape_survives_the_cache() {
    check_round_trip(FIXTURE);
}
