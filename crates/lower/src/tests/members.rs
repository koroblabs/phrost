use expect_test::expect;

use super::check_hir;

#[test]
fn visibility_defaults_to_public_and_modifiers_are_recorded() {
    check_hir(
        r#"
//- /src/C.php
<?php
abstract class C
{
    function implicitlyPublic() {}
    private static function hidden() {}
    abstract protected function todo(): void;
    final public function sealed() {}
}
"#,
        expect![[r#"
            class C abstract @ 2:16-2:17
              method implicitlyPublic()  public @ 4:14-4:30
              method hidden()  private static @ 5:29-5:35
              method todo(): void  protected abstract @ 6:33-6:37
              method sealed()  public final @ 7:27-7:33
        "#]],
    );
}

#[test]
fn one_property_declaration_can_declare_several_properties() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    private int $a, $b;
    public readonly string $name;
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              property $a: int  private @ 4:17-4:19
              property $b: int  private @ 4:21-4:23
              property $name: string  public readonly @ 5:28-5:33
        "#]],
    );
}

#[test]
fn promoted_constructor_parameters_also_become_properties() {
    // Emitting the property here means member lookup never has to know that constructor property
    // promotion exists.
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function __construct(
        private readonly int $id,
        protected ?string $name = null,
        public array ...$rest,
    ) {}
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method __construct(private readonly int $id, protected ?string $name = ..., public array ...$rest)  public @ 4:21-4:32
              property $id: int  private readonly @ 5:30-5:33
              property $name: ?string  protected @ 6:27-6:32
              property $rest: array  public @ 7:25-7:30
        "#]],
    );
}

#[test]
fn asymmetric_visibility_records_both_halves() {
    // PHP 8.4. Getting this wrong reports a property as writable when it is not.
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public private(set) int $id;
    public protected(set) string $slug;
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              property $id: int  public private(set) @ 4:29-4:32
              property $slug: string  public protected(set) @ 5:34-5:39
        "#]],
    );
}

#[test]
fn property_hooks_are_flagged() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public string $name { get => 'x'; set { } }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              property $name: string  public hooks @ 4:19-4:24
                hook get() @ 4:27-4:30
                hook set(string $value) @ 4:39-4:42
        "#]],
    );
}

#[test]
fn typed_class_constants_keep_their_type() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    const string VERSION = '1.0';
    private const int MAX = 10, MIN = 1;
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              const VERSION: string  public @ 4:18-4:25
              const MAX: int  private @ 5:23-5:26
              const MIN: int  private @ 5:33-5:36
        "#]],
    );
}

#[test]
fn parameters_record_defaults_variadics_and_references() {
    check_hir(
        r#"
//- /src/C.php
<?php
function f(int $a, string $b = 'x', ?User &$c = null, int ...$rest) {}
"#,
        expect![[r#"
            function f(int $a, string $b = ..., ?User &$c = ..., int ...$rest) @ 2:10-2:11
        "#]],
    );
}

#[test]
fn namespace_level_constants_split_per_name() {
    check_hir(
        r#"
//- /src/C.php
<?php
namespace App;
const A = 1, B = 2;
"#,
        expect![[r#"
            namespace App @ 2:11-2:14
              const A @ 3:7-3:8
              const B @ 3:14-3:15
        "#]],
    );
}
