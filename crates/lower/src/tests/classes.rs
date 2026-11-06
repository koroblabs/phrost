use expect_test::expect;

use super::check_hir;

#[test]
fn class_with_inheritance_and_members() {
    check_hir(
        r#"
//- /src/Repo.php
<?php
namespace App;

abstract class Repo extends Base implements Countable, \JsonSerializable
{
    public function find(int $id): ?User {}
}
"#,
        expect![[r#"
            namespace App @ 2:11-2:14
              class Repo abstract @ 4:16-4:20
                extends Base
                implements Countable, \JsonSerializable
                method find(int $id): ?User  public @ 6:21-6:25
        "#]],
    );
}

#[test]
fn interface_extends_several_interfaces() {
    check_hir(
        r#"
//- /src/I.php
<?php
interface Repo extends Countable, IteratorAggregate {}
"#,
        expect![[r#"
            interface Repo @ 2:11-2:15
              extends Countable, IteratorAggregate
        "#]],
    );
}

#[test]
fn final_and_readonly_class_modifiers() {
    check_hir(
        r#"
//- /src/C.php
<?php
final readonly class Money {}
"#,
        expect![[r#"
            class Money final readonly @ 2:22-2:27
        "#]],
    );
}

#[test]
fn trait_use_with_conflict_resolution() {
    check_hir(
        r#"
//- /src/T.php
<?php
class C
{
    use A, B {
        A::run insteadof B;
        B::run as protected legacyRun;
        helper as private;
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              uses A, B
              adapt A::run insteadof B
              adapt B::run as protected legacyRun
              adapt helper as private
        "#]],
    );
}

#[test]
fn backed_enum_with_cases_and_interface() {
    check_hir(
        r#"
//- /src/Suit.php
<?php
enum Suit: string implements JsonSerializable
{
    case Hearts = 'H';
    case Spades = 'S';

    public function color(): string {}
}
"#,
        expect![[r#"
            enum Suit: string @ 2:6-2:10
              implements JsonSerializable
              case Hearts  public @ 4:10-4:16
              case Spades  public @ 5:10-5:16
              method color(): string  public @ 7:21-7:26
        "#]],
    );
}

#[test]
fn several_namespaces_in_one_file_each_own_their_declarations() {
    // A file can declare more than one namespace, and the same short name resolves differently in
    // each. The namespace ranges are what let a later layer answer "which one is this position in".
    check_hir(
        r#"
//- /src/Two.php
<?php
namespace First {
    class A {}
}
namespace Second {
    class B {}
}
"#,
        expect![[r#"
            namespace First @ 2:11-2:16
              class A @ 3:11-3:12
            namespace Second @ 5:11-5:17
              class B @ 6:11-6:12
        "#]],
    );
}

#[test]
fn declarations_in_a_bare_block_are_still_declarations() {
    check_hir(
        r#"
//- /src/Block.php
<?php
{
    class Inner {}
}
"#,
        expect![[r#"
            class Inner @ 3:11-3:16
        "#]],
    );
}

/// Both classes, and the errors that say the file is still broken.
///
/// `Bad` used to be missing here, and this test's own name said it should not be: a class the person
/// is in the middle of writing is a class the rest of the project already refers to, and losing it
/// makes every `new Bad()` elsewhere read as undefined. What recovered it is the parse repair --
/// mago asks for a `}` at end of file, one is supplied, and the class survives with the unnamed
/// method it could not finish dropped.
///
/// The errors are unchanged, which is the half worth pinning: a repair changes what the tree holds
/// and never what is wrong with the source.
#[test]
fn a_broken_file_still_lowers_what_parsed() {
    check_hir(
        r#"
//- /src/Broken.php
<?php
class Good {}
class Bad { public function
"#,
        expect![[r#"
            class Good @ 2:7-2:11
            class Bad @ 3:7-3:10
            !! Unexpected end of file
            !! Unexpected end of file
            !! Expected one of `RightBrace` before end of file
        "#]],
    );
}

#[test]
fn two_files_share_one_interner() {
    check_hir(
        r#"
//- /src/A.php
<?php
namespace App;
class A {}

//- /src/B.php
<?php
namespace App;
class B extends A {}
"#,
        expect![[r#"
            // src/A.php
            namespace App @ 2:11-2:14
              class A @ 3:7-3:8
            // src/B.php
            namespace App @ 2:11-2:14
              class B @ 3:7-3:8
                extends A
        "#]],
    );
}
