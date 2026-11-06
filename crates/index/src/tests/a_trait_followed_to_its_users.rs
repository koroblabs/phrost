//! The files composing each trait, which the skeleton keeps beside the rows so `$this` in a trait
//! can be read as its hosts without walking the class table.
//!
//! Kept by `install` and `retract`, so an edit that takes a `use` away has to take the file out of
//! the map and one that adds it has to put it in -- each line a re-ingest could forget, which is
//! what the dump holds against a fresh index.

use expect_test::expect;

use super::{Step, check_incremental};

#[test]
fn a_use_moved_from_one_class_to_another_moves_the_file() {
    check_incremental(
        r#"
//- /src/Counts.php
<?php
namespace App;
trait Counts {}

//- /src/A.php
<?php
namespace App;
class A { use Counts; }

//- /src/B.php
<?php
namespace App;
class B {}

//- /src/C.php
<?php
namespace App;
class C { use Counts; }
"#,
        &[
            Step::write("src/A.php", "<?php\nnamespace App;\nclass A {}\n"),
            Step::write(
                "src/B.php",
                "<?php\nnamespace App;\nclass B { use \\App\\Counts; }\n",
            ),
            Step::delete("src/C.php"),
        ],
        expect![[r#"
            state ready
            file src/A.php [User]
              class App\A @ 3:7-3:8
              App\A is class
            file src/B.php [User]
              class App\B @ 3:7-3:8
              App\B is class
              App\B uses App\Counts @ 3:15-3:26
            file src/Counts.php [User]
              trait App\Counts @ 3:7-3:13
              App\Counts is trait
            catalog
              [class] app\a -> src/A.php (App\A)
              [class] app\b -> src/B.php (App\B)
              [class] app\counts -> src/Counts.php (App\Counts)
            namespaces
              app -> App x3
            declared keys
            dispatched keys
            trait users
              app\counts -> src/B.php
        "#]],
    );
}
