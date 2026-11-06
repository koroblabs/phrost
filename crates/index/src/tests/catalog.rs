use expect_test::expect;

use super::check_index;

#[test]
fn declarations_are_catalogued_under_their_folded_fqn() {
    check_index(
        r#"
//- /src/Repo.php
<?php
namespace App\Data;

class Repo {}
interface Contract {}
function make() {}
const LIMIT = 1;
"#,
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Data\Repo @ 4:7-4:11
              interface App\Data\Contract @ 5:11-5:19
              function App\Data\make @ 6:10-6:14
              constant App\Data\LIMIT @ 7:7-7:12
              function App\Data\make()
              constant App\Data\LIMIT: int
              App\Data\Repo is class
              App\Data\Contract is interface
            catalog
              [class] app\data\contract -> src/Repo.php (App\Data\Contract)
              [class] app\data\repo -> src/Repo.php (App\Data\Repo)
              [const] App\Data\LIMIT -> src/Repo.php (App\Data\LIMIT)
              [function] app\data\make -> src/Repo.php (App\Data\make)
            namespaces
              app -> App x4
              app\data -> App\Data x4
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn constants_are_catalogued_case_sensitively() {
    // PHP compares constant names exactly, so `MAX` and `max` are two constants. A single folded
    // table would silently merge them and answer one lookup with the other's location.
    check_index(
        r#"
//- /src/C.php
<?php
namespace App;
const MAX = 1;
const max = 2;
"#,
        expect![[r#"
            state ready
            file src/C.php [User]
              constant App\MAX @ 3:7-3:10
              constant App\max @ 4:7-4:10
              constant App\MAX: int
              constant App\max: int
            catalog
              [const] App\MAX -> src/C.php (App\MAX)
              [const] App\max -> src/C.php (App\max)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn class_and_function_tables_do_not_collide() {
    // `App\render` can be both a class and a function at the same time. One table would make them
    // fight over the same key.
    check_index(
        r#"
//- /src/C.php
<?php
namespace App;
class render {}
function render() {}
"#,
        expect![[r#"
            state ready
            file src/C.php [User]
              class App\render @ 3:7-3:13
              function App\render @ 4:10-4:16
              function App\render()
              App\render is class
            catalog
              [class] app\render -> src/C.php (App\render)
              [function] app\render -> src/C.php (App\render)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn user_code_outranks_vendor_for_the_same_name() {
    // A project can legitimately redeclare a dependency's class -- a polyfill, or a forked package.
    // The copy the developer is editing is the one they mean.
    check_index(
        r#"
//- /vendor/acme/src/Repo.php
<?php
namespace App;
class Repo {}

//- /src/Repo.php
<?php
namespace App;
class Repo {}
"#,
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Repo @ 3:7-3:11
              App\Repo is class
            file vendor/acme/src/Repo.php [Vendor]
              class App\Repo @ 3:7-3:11
              App\Repo is class
            catalog
              [class] app\repo -> src/Repo.php (App\Repo) | vendor/acme/src/Repo.php (App\Repo)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_stub_is_outranked_by_anything_that_really_declares_the_name() {
    // The standard library is indexed like any other source, so a project that polyfills a class the
    // stubs also describe would otherwise get two answers with no reason to prefer either. A stub
    // describes what the runtime provides; a real file is what the code actually runs.
    check_index(
        r#"
//- /stubs/core.php  stub
<?php
class ArrayObject {}

//- /vendor/acme/src/ArrayObject.php
<?php
class ArrayObject {}

//- /src/ArrayObject.php
<?php
class ArrayObject {}
"#,
        expect![[r#"
            state ready
            file src/ArrayObject.php [User]
              class ArrayObject @ 2:7-2:18
              ArrayObject is class
            file stubs/core.php [Stub]
              class ArrayObject @ 2:7-2:18
              ArrayObject is class
            file vendor/acme/src/ArrayObject.php [Vendor]
              class ArrayObject @ 2:7-2:18
              ArrayObject is class
            catalog
              [class] arrayobject -> src/ArrayObject.php (ArrayObject) | vendor/acme/src/ArrayObject.php (ArrayObject) | stubs/core.php (ArrayObject)
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn equal_origins_are_ranked_by_path() {
    // The tie-break has to be something stable across builds. Path is; `FileId` is not, because it
    // reflects the order files happened to be discovered.
    check_index(
        r#"
//- /src/b/Repo.php
<?php
namespace App;
class Repo {}

//- /src/a/Repo.php
<?php
namespace App;
class Repo {}
"#,
        expect![[r#"
            state ready
            file src/a/Repo.php [User]
              class App\Repo @ 3:7-3:11
              App\Repo is class
            file src/b/Repo.php [User]
              class App\Repo @ 3:7-3:11
              App\Repo is class
            catalog
              [class] app\repo -> src/a/Repo.php (App\Repo) | src/b/Repo.php (App\Repo)
            namespaces
              app -> App x2
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn inheritance_edges_resolve_through_the_files_imports() {
    check_index(
        r#"
//- /src/Repo.php
<?php
namespace App;

use Vendor\Base;
use Vendor\Contracts\Countable as Counter;

class Repo extends Base implements Counter
{
}
"#,
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Repo @ 7:7-7:11
              use Vendor\Base as Base  [class]
              use Vendor\Contracts\Countable as Counter  [class]
              App\Repo is class
              App\Repo extends Vendor\Base @ 7:20-7:24
              App\Repo implements Vendor\Contracts\Countable @ 7:36-7:43
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
fn an_unresolvable_parent_still_records_what_it_would_have_been() {
    // The candidate is a pure function of this file's namespace and imports, so it is recorded
    // whether or not the target exists. That is what lets a later layer report "Missing is not
    // declared" instead of having nothing to say.
    check_index(
        r#"
//- /src/Repo.php
<?php
namespace App;
class Repo extends Missing {}
"#,
        expect![[r#"
            state ready
            file src/Repo.php [User]
              class App\Repo @ 3:7-3:11
              App\Repo is class
              App\Repo extends App\Missing @ 3:20-3:27
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
fn a_file_declaring_nothing_still_appears() {
    check_index(
        r#"
//- /src/Empty.php
<?php
// nothing here
"#,
        expect![[r#"
            state ready
            file src/Empty.php [User]
            catalog
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}
