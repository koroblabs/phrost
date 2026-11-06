use expect_test::expect;

use super::check_scope;

#[test]
fn declarations_are_qualified_by_their_namespace() {
    check_scope(
        r#"
//- /src/Repo.php
<?php
namespace App\Data;

class Repo {}
interface Contract {}
trait Helper {}
enum Status {}
function make() {}
const LIMIT = 10;
"#,
        expect![[r#"
            scope App\Data
              class App\Data\Repo @ 4:7-4:11
              interface App\Data\Contract @ 5:11-5:19
              trait App\Data\Helper @ 6:7-6:13
              enum App\Data\Status @ 7:6-7:12
              function App\Data\make @ 8:10-8:14
              constant App\Data\LIMIT @ 9:7-9:12
        "#]],
    );
}

#[test]
fn the_global_namespace_needs_no_prefix() {
    check_scope(
        r#"
//- /src/Global.php
<?php
class Repo {}
function make() {}
"#,
        expect![[r#"
            scope <global>
              class Repo @ 2:7-2:11
              function make @ 3:10-3:14
        "#]],
    );
}

#[test]
fn imports_belong_to_the_namespace_that_declared_them() {
    // An import inside one brace-delimited namespace does not apply in the next one. Getting this
    // wrong makes a name resolve in a block that never imported it.
    check_scope(
        r#"
//- /src/Two.php
<?php
namespace First {
    use Vendor\Alpha;
    class A {}
}
namespace Second {
    use Vendor\Beta;
    class B {}
}
"#,
        expect![[r#"
            scope First
              use Vendor\Alpha  [class]
              class First\A @ 4:11-4:12
            scope Second
              use Vendor\Beta  [class]
              class Second\B @ 8:11-8:12
        "#]],
    );
}

#[test]
fn imports_are_split_across_the_three_symbol_tables() {
    check_scope(
        r#"
//- /src/Use.php
<?php
namespace App;

use Vendor\Repo;
use function Vendor\render;
use const Vendor\MAX;
"#,
        expect![[r#"
            scope App
              use Vendor\Repo  [class]
              use Vendor\render  [function]
              use Vendor\MAX  [const]
        "#]],
    );
}

#[test]
fn an_explicit_alias_is_shown_and_an_implicit_one_is_not() {
    check_scope(
        r#"
//- /src/Use.php
<?php
use Vendor\Contracts\RepoInterface as Contract;
use Vendor\Repo;
class Thing {}
"#,
        expect![[r#"
            scope <global>
              use Vendor\Contracts\RepoInterface as Contract  [class]
              use Vendor\Repo  [class]
              class Thing @ 4:7-4:12
        "#]],
    );
}

#[test]
fn several_files_declaring_the_same_short_name_get_different_fqns() {
    check_scope(
        r#"
//- /src/A/Repo.php
<?php
namespace App\A;
class Repo {}

//- /src/B/Repo.php
<?php
namespace App\B;
class Repo {}
"#,
        expect![[r#"
            // src/A/Repo.php
            scope App\A
              class App\A\Repo @ 3:7-3:11
            // src/B/Repo.php
            scope App\B
              class App\B\Repo @ 3:7-3:11
        "#]],
    );
}
