//! The `incremental == from-scratch` invariant.
//!
//! Every test here applies edits to an indexed project and proves the result is identical to
//! indexing the final state directly. The from-scratch build assigns file ids in the opposite
//! order, so a dependency on discovery order fails here rather than surfacing later as an
//! occasional wrong answer.

use expect_test::expect;

use super::{Step, check_incremental};

#[test]
fn replacing_a_files_contents_matches_a_fresh_index() {
    check_incremental(
        r#"
//- /src/A.php
<?php
namespace App;
class A {}
"#,
        // The edit adds a `@template` and an `@assert-if-true`, so the invariant covers the two
        // fields a re-ingest could drop without anything else noticing -- the same two the cache
        // round trip covers.
        &[Step::write(
            "src/A.php",
            "<?php\nnamespace App;\nclass A {}\n/** @template T of A */\ninterface B {\n  /** @assert-if-true \\App\\A $this */\n  public function isA(): bool;\n}\n",
        )],
        expect![[r#"
            state ready
            file src/A.php [User]
              class App\A @ 3:7-3:8
              interface App\B @ 5:11-5:12
              App\A is class
              App\B is interface
              App\B template T of A
              App\B::method isA(): bool  public asserts \App\A if true  @ 7:19-7:22 (full 7:3-7:31)
            catalog
              [class] app\a -> src/A.php (App\A)
              [class] app\b -> src/A.php (App\B)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_throws_tag_added_by_an_edit_is_the_same_as_having_had_it_all_along() {
    // Its own test rather than another tag on the edit above, because what it exercises is a
    // *sparse table* and not a field on a row: a declaration with no tag has no entry at all, so
    // both directions are walked -- one tag added where there was none, and one dropped where
    // there was one.
    //
    // Today the second direction cannot fail, and the test says so on purpose: the table travels on
    // `FileSkeleton`, so `install` replacing the file's rows takes the old entries with it. What
    // this guards is the day it moves to a file-keyed side table beside `dispatches`, where
    // withdrawal is a line somebody has to write in `retract` and where §8 records what forgetting
    // it costs.
    check_incremental(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * @throws \RuntimeException
     */
    public function find(): void {}

    public function all(): void {}
}
"#,
        &[Step::write(
            "src/Repo.php",
            "<?php
namespace App;

class Repo
{
    public function find(): void {}

    /**
     * @throws \\LogicException
     */
    public function all(): void {}
}
",
        )],
        expect![[r#"
            state ready
            file src/Repo.php [User]
              App\Repo::all throws LogicException
              class App\Repo @ 4:7-4:11
              App\Repo is class
              App\Repo::method find(): void  public  @ 6:21-6:25 (full 6:5-6:36)
              App\Repo::method all(): void  public  @ 11:21-11:24 (full 11:5-11:35)
            catalog
              [class] app\repo -> src/Repo.php (App\Repo)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_body_derived_return_and_a_default_both_ways_match_a_fresh_index() {
    // Both directions for each, because both are sparse: a chain added where the declaration used
    // to name its type, and a default removed where a parameter used to carry one. A re-ingest that
    // only ever added would pass the first of each and leave a stale row.
    check_incremental(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Row {}

class Repo
{
    public function first(): Row {}

    public function find(int $id = 1): Row {}
}
"#,
        &[Step::write(
            "src/Repo.php",
            "<?php\nnamespace App;\n\nclass Row {}\n\nclass Repo\n{\n    public function first()\n    {\n        return $this->find()->row;\n    }\n\n    public function find(): Row {}\n}\n",
        )],
        expect![[r#"
            state ready
            file src/Repo.php [User]
              App\Repo::first returns $this->find->row
              class App\Row @ 4:7-4:10
              class App\Repo @ 6:7-6:11
              App\Row is class
              App\Repo is class
              App\Repo::method first()  public  @ 8:21-8:26 (full 8:5-11:6)
              App\Repo::method find(): Row  public  @ 13:21-13:25 (full 13:5-13:35)
            catalog
              [class] app\repo -> src/Repo.php (App\Repo)
              [class] app\row -> src/Repo.php (App\Row)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn renaming_a_class_removes_the_old_entry() {
    // The failure this guards against is an update that only adds: the old name keeps resolving to
    // a location that no longer declares it, and goto lands on unrelated code.
    check_incremental(
        r#"
//- /src/A.php
<?php
namespace App;
class Old {}
"#,
        &[Step::write(
            "src/A.php",
            "<?php\nnamespace App;\nclass New {}\n",
        )],
        expect![[r#"
            state ready
            file src/A.php [User]
              class App\New @ 3:7-3:10
              App\New is class
            catalog
              [class] app\new -> src/A.php (App\New)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn deleting_a_file_removes_everything_it_declared() {
    check_incremental(
        r#"
//- /src/A.php
<?php
namespace App;
class A {}

//- /src/B.php
<?php
namespace App;
class B {}
"#,
        &[Step::delete("src/B.php")],
        expect![[r#"
            state ready
            file src/A.php [User]
              class App\A @ 3:7-3:8
              App\A is class
            catalog
              [class] app\a -> src/A.php (App\A)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn adding_a_file_is_the_same_as_having_had_it_all_along() {
    check_incremental(
        r#"
//- /src/A.php
<?php
namespace App;
class A {}
"#,
        &[Step::write(
            "src/C.php",
            "<?php\nnamespace App;\nclass C extends A {}\n",
        )],
        expect![[r#"
            state ready
            file src/A.php [User]
              class App\A @ 3:7-3:8
              App\A is class
            file src/C.php [User]
              class App\C @ 3:7-3:8
              App\C is class
              App\C extends App\A @ 3:17-3:18
            catalog
              [class] app\a -> src/A.php (App\A)
              [class] app\c -> src/C.php (App\C)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn breaking_and_repairing_syntax_leaves_no_trace() {
    // An editor buffer is invalid most of the time it is being typed into, so a broken revision
    // must not leave anything behind once it is fixed.
    check_incremental(
        r#"
//- /src/A.php
<?php
namespace App;
class A {}
"#,
        &[
            Step::write(
                "src/A.php",
                "<?php\nnamespace App;\nclass A { public function\n",
            ),
            Step::write("src/A.php", "<?php\nnamespace App;\nclass A {}\n"),
        ],
        expect![[r#"
            state ready
            file src/A.php [User]
              class App\A @ 3:7-3:8
              App\A is class
            catalog
              [class] app\a -> src/A.php (App\A)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn changing_an_import_changes_what_the_parent_resolves_to() {
    check_incremental(
        r#"
//- /src/Repo.php
<?php
namespace App;
use Vendor\Base;
class Repo extends Base {}
"#,
        &[Step::write(
            "src/Repo.php",
            "<?php\nnamespace App;\nuse Other\\Base;\nclass Repo extends Base {}\n",
        )],
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Repo @ 4:7-4:11
              use Other\Base as Base  [class]
              App\Repo is class
              App\Repo extends Other\Base @ 4:20-4:24
            catalog
              [class] app\repo -> src/Repo.php (App\Repo)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn removing_the_user_copy_promotes_the_vendor_one() {
    // Conflict ranking has to be recomputed on read, not frozen at ingest: deleting the winner
    // must promote the runner-up rather than leave the name unowned.
    check_incremental(
        r#"
//- /src/Repo.php
<?php
namespace App;
class Repo {}

//- /vendor/acme/Repo.php
<?php
namespace App;
class Repo {}
"#,
        &[Step::delete("src/Repo.php")],
        expect![[r#"
            state ready
            file vendor/acme/Repo.php [Vendor]
              class App\Repo @ 3:7-3:11
              App\Repo is class
            catalog
              [class] app\repo -> vendor/acme/Repo.php (App\Repo)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn moving_a_class_between_namespaces_moves_its_catalog_entry() {
    check_incremental(
        r#"
//- /src/A.php
<?php
namespace App\First;
class Thing {}
"#,
        &[Step::write(
            "src/A.php",
            "<?php\nnamespace App\\Second;\nclass Thing {}\n",
        )],
        expect![[r#"
            state ready
            file src/A.php [User]
              class App\Second\Thing @ 3:7-3:12
              App\Second\Thing is class
            catalog
              [class] app\second\thing -> src/A.php (App\Second\Thing)
            namespaces
              app -> App x1
              app\second -> App\Second x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn changing_an_attributes_key_moves_what_it_claims() {
    // The invalidation the `entity` catalog rests on. A key is not a name PHP has, so nothing in
    // the symbol catalog withdraws it -- if the claim were derived one way on the way in and another
    // on the way out, the old id would keep naming a class that no longer declares it, and
    // `getStorage('old')` would resolve for hours after the rename.
    check_incremental(
        r#"
//- /src/A.php
<?php
namespace App;
#[Plugin(id: 'old_name')]
class A {}
"#,
        &[Step::write(
            "src/A.php",
            "<?php\nnamespace App;\n#[Plugin(id: 'new_name')]\nclass A {}\n",
        )],
        expect![[r#"
            state ready
            file src/A.php [User]
              class App\A @ 4:7-4:8
              App\A is class
            catalog
              [class] app\a -> src/A.php (App\A)
            namespaces
              app -> App x1
            declared keys
              [plugin] new_name -> App\A
            dispatched keys
        "#]],
    );
}

#[test]
fn deleting_the_file_takes_its_claim_with_it() {
    check_incremental(
        r#"
//- /src/A.php
<?php
namespace App;
#[Plugin(id: 'thing')]
class A {}

//- /src/B.php
<?php
namespace App;
#[Plugin(id: 'other')]
class B {}
"#,
        &[Step::delete("src/A.php")],
        expect![[r#"
            state ready
            file src/B.php [User]
              class App\B @ 4:7-4:8
              App\B is class
            catalog
              [class] app\b -> src/B.php (App\B)
            namespaces
              app -> App x1
            declared keys
              [plugin] other -> App\B
            dispatched keys
        "#]],
    );
}
