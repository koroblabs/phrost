use expect_test::expect;

use super::check_hir;

#[test]
fn plain_and_aliased_class_imports() {
    check_hir(
        r#"
//- /src/Use.php
<?php
use App\Repo;
use App\Contracts\RepoInterface as Contract;
"#,
        expect![[r#"
            use App\Repo  [class] @ 2:5-2:13
            use App\Contracts\RepoInterface as Contract  [class] @ 3:5-3:32
        "#]],
    );
}

#[test]
fn function_and_constant_imports_use_separate_tables() {
    // PHP keeps three symbol tables, so `use App\render` and `use function App\render` neither
    // collide nor substitute for one another.
    check_hir(
        r#"
//- /src/Use.php
<?php
use App\render;
use function App\render;
use const App\MAX;
"#,
        expect![[r#"
            use App\render  [class] @ 2:5-2:15
            use App\render  [function] @ 3:14-3:24
            use App\MAX  [const] @ 4:11-4:18
        "#]],
    );
}

#[test]
fn group_imports_are_flattened() {
    check_hir(
        r#"
//- /src/Use.php
<?php
use App\{Repo, User as Person};
use function App\Support\{first, second};
"#,
        expect![[r#"
            use App\Repo  [class] @ 2:10-2:14
            use App\User as Person  [class] @ 2:16-2:20
            use App\Support\first  [function] @ 3:27-3:32
            use App\Support\second  [function] @ 3:34-3:40
        "#]],
    );
}

#[test]
fn a_mixed_group_import_carries_a_kind_per_item() {
    check_hir(
        r#"
//- /src/Use.php
<?php
use App\{Repo, function render, const MAX};
"#,
        expect![[r#"
            use App\Repo  [class] @ 2:10-2:14
            use App\render  [function] @ 2:25-2:31
            use App\MAX  [const] @ 2:39-2:42
        "#]],
    );
}

#[test]
fn an_import_is_absolute_even_without_a_leading_separator() {
    // `use A\B;` and `use \A\B;` mean the same thing: a use statement is never relative to the
    // current namespace. Both are stored the same way so no consumer has to remember that.
    check_hir(
        r#"
//- /src/Use.php
<?php
namespace App;
use Other\Thing;
use \Other\Thing as Same;
"#,
        expect![[r#"
            namespace App @ 2:11-2:14
              use Other\Thing  [class] @ 3:5-3:16
              use Other\Thing as Same  [class] @ 4:5-4:17
        "#]],
    );
}
