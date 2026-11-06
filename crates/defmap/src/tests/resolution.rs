use expect_test::expect;
use phrost_hir::{ImportKind, NameAnchor};

use super::{check_lookup, check_resolution};

#[test]
fn an_unqualified_class_name_resolves_in_the_current_namespace() {
    check_resolution(
        r#"
//- /src/T.php
<?php
namespace App;

class C
{
    public function a(): Re@plain@po {}
}
"#,
        &["plain"],
        expect![[r#"
            plain: Repo (Relative)
              App\Repo via CurrentNamespace
        "#]],
    );
}

#[test]
fn an_import_beats_the_current_namespace() {
    check_resolution(
        r#"
//- /src/T.php
<?php
namespace App;

use Vendor\Repo;

class C
{
    public function a(): Re@imported@po {}
}
"#,
        &["imported"],
        expect![[r#"
            imported: Repo (Relative)
              Vendor\Repo via Import(ImportId(0))
        "#]],
    );
}

#[test]
fn an_import_alias_matches_case_insensitively() {
    // Class names are case-insensitive in PHP, so an import bound as `Repo` also answers `repo`.
    check_resolution(
        r#"
//- /src/T.php
<?php
namespace App;

use Vendor\Repo;

class C
{
    public function a(): re@folded@po {}
}
"#,
        &["folded"],
        expect![[r#"
            folded: repo (Relative)
              Vendor\Repo via Import(ImportId(0))
        "#]],
    );
}

#[test]
fn a_fully_qualified_name_ignores_imports_entirely() {
    check_resolution(
        r#"
//- /src/T.php
<?php
namespace App;

use Vendor\Repo;

class C
{
    public function a(): \Other\Re@absolute@po {}
}
"#,
        &["absolute"],
        expect![[r#"
            absolute: Other\Repo (FullyQualified)
              Other\Repo via Absolute
        "#]],
    );
}

#[test]
fn a_qualified_name_translates_only_its_first_segment() {
    check_resolution(
        r#"
//- /src/T.php
<?php
namespace App;

use Vendor\Support;

class C
{
    public function a(): Sup@via_import@port\Inner\Thing {}
    public function b(): Unk@via_namespace@nown\Thing {}
}
"#,
        &["via_import", "via_namespace"],
        expect![[r#"
            via_import: Support\Inner\Thing (Relative)
              Vendor\Support\Inner\Thing via Import(ImportId(0))
            via_namespace: Unknown\Thing (Relative)
              App\Unknown\Thing via CurrentNamespace
        "#]],
    );
}

#[test]
fn the_namespace_operator_skips_imports() {
    check_resolution(
        r#"
//- /src/T.php
<?php
namespace App;

use Vendor\Repo;

class C
{
    public function a(): namespace\Re@explicit@po {}
}
"#,
        &["explicit"],
        expect![[r#"
            explicit: Repo (CurrentNamespace)
              App\Repo via CurrentNamespace
        "#]],
    );
}

#[test]
fn each_namespace_block_resolves_with_its_own_imports() {
    check_resolution(
        r#"
//- /src/T.php
<?php
namespace First {
    use Vendor\Alpha as Shared;
    class A
    {
        public function a(): Sha@first@red {}
    }
}
namespace Second {
    use Vendor\Beta as Shared;
    class B
    {
        public function b(): Sha@second@red {}
    }
}
"#,
        &["first", "second"],
        expect![[r#"
            first: Shared (Relative)
              Vendor\Alpha via Import(ImportId(0))
            second: Shared (Relative)
              Vendor\Beta via Import(ImportId(1))
        "#]],
    );
}

#[test]
fn only_functions_and_constants_fall_back_to_the_global_namespace() {
    // The asymmetry that causes wrong answers if forgotten. `new Repo()` inside a namespace never
    // looks at `\Repo`, but `count()` does look at `\count()`.
    check_lookup(
        r#"
//- /src/T.php
<?php
namespace App;

class C
{
    public function a(): Mar@here@ker {}
}
"#,
        "here",
        &[
            ("Repo", NameAnchor::Relative, ImportKind::ClassLike),
            ("render", NameAnchor::Relative, ImportKind::Function),
            ("MAX", NameAnchor::Relative, ImportKind::Constant),
        ],
        expect![[r#"
            Repo (Relative, class)
              App\Repo via CurrentNamespace
            render (Relative, function)
              App\render via CurrentNamespace
              render via Global
            MAX (Relative, const)
              App\MAX via CurrentNamespace
              MAX via Global
        "#]],
    );
}

#[test]
fn a_qualified_name_never_falls_back_to_global() {
    check_lookup(
        r#"
//- /src/T.php
<?php
namespace App;

class C
{
    public function a(): Mar@here@ker {}
}
"#,
        "here",
        &[
            (
                "Support\\render",
                NameAnchor::Relative,
                ImportKind::Function,
            ),
            ("Support\\MAX", NameAnchor::Relative, ImportKind::Constant),
        ],
        expect![[r#"
            Support\render (Relative, function)
              App\Support\render via CurrentNamespace
            Support\MAX (Relative, const)
              App\Support\MAX via CurrentNamespace
        "#]],
    );
}

#[test]
fn a_constant_import_matches_case_sensitively_but_a_function_import_does_not() {
    // PHP folds class and function names but not constants, so `use const App\MAX;` does not make
    // `max` resolve. Folding constants too would resolve names PHP would not.
    check_lookup(
        r#"
//- /src/T.php
<?php
namespace App;

use function Vendor\Render;
use const Vendor\MAX;

class C
{
    public function a(): Mar@here@ker {}
}
"#,
        "here",
        &[
            ("render", NameAnchor::Relative, ImportKind::Function),
            ("MAX", NameAnchor::Relative, ImportKind::Constant),
            ("max", NameAnchor::Relative, ImportKind::Constant),
        ],
        expect![[r#"
            render (Relative, function)
              Vendor\Render via Import(ImportId(0))
            MAX (Relative, const)
              Vendor\MAX via Import(ImportId(1))
            max (Relative, const)
              App\max via CurrentNamespace
              max via Global
        "#]],
    );
}

#[test]
fn a_qualified_name_uses_the_class_table_even_for_a_function_lookup() {
    // `use App\Support;` imports a namespace through the class table, and `Support\render()` then
    // works. A function lookup that consulted only the function table would miss it.
    check_lookup(
        r#"
//- /src/T.php
<?php
namespace App;

use Vendor\Support;

class C
{
    public function a(): Mar@here@ker {}
}
"#,
        "here",
        &[(
            "Support\\render",
            NameAnchor::Relative,
            ImportKind::Function,
        )],
        expect![[r#"
            Support\render (Relative, function)
              Vendor\Support\render via Import(ImportId(0))
        "#]],
    );
}

#[test]
fn in_the_global_namespace_there_is_nothing_to_fall_back_to() {
    check_lookup(
        r#"
//- /src/T.php
<?php
class C
{
    public function a(): Mar@here@ker {}
}
"#,
        "here",
        &[
            ("Repo", NameAnchor::Relative, ImportKind::ClassLike),
            ("render", NameAnchor::Relative, ImportKind::Function),
        ],
        expect![[r#"
            Repo (Relative, class)
              Repo via Global
            render (Relative, function)
              render via Global
        "#]],
    );
}
