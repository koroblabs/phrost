use expect_test::expect;

use super::check_hir;

#[test]
fn a_docblock_attaches_to_the_declaration_below_it() {
    check_hir(
        r#"
//- /src/C.php
<?php
/** The repository. */
class Repo
{
    /** Finds one. */
    public function find(): void {}

    public function undocumented(): void {}
}

/** A free function. */
function go(): void {}
"#,
        expect![[r#"
            class Repo @ 3:7-3:11  [doc]
              method find(): void  public @ 6:21-6:25  [doc]
              method undocumented(): void  public @ 8:21-8:33
            function go(): void @ 12:10-12:12  [doc]
        "#]],
    );
}

#[test]
fn a_docblock_above_an_attribute_still_attaches() {
    // `#[Attr]` sits between the comment and the keyword. Measuring from the keyword would lose
    // the docblock on every attributed declaration, which in modern PHP is most of them.
    check_hir(
        r#"
//- /src/C.php
<?php
/** Documented. */
#[Attribute]
class Repo {}
"#,
        expect![[r#"
            class Repo @ 4:7-4:11  [doc]
        "#]],
    );
}

#[test]
fn a_plain_comment_is_not_documentation() {
    check_hir(
        r#"
//- /src/C.php
<?php
/* just a note */
// and a line
class Repo {}
"#,
        expect![[r#"
            class Repo @ 4:7-4:11
        "#]],
    );
}

#[test]
fn a_docblock_separated_by_code_does_not_attach() {
    check_hir(
        r#"
//- /src/C.php
<?php
/** About something else. */
$x = 1;
class Repo {}
"#,
        expect![[r#"
            class Repo @ 4:7-4:11
        "#]],
    );
}

#[test]
fn a_docblock_attaches_across_modifiers_and_attributes() {
    // The declaration does not start at its keyword: `abstract`, `public`, and `#[Attr]` all come
    // first. Measuring from the keyword leaves those in the gap and silently drops the docblock --
    // which is what this originally did, for every method in the codebase.
    check_hir(
        r#"
//- /src/C.php
<?php
/** The class. */
#[Attribute]
abstract class C
{
    /** The constant. */
    private const int MAX = 1;

    /** The property. */
    #[Attr]
    protected readonly string $name;

    /** The method. */
    abstract public static function make(): static;
}
"#,
        expect![[r#"
            class C abstract @ 4:16-4:17  [doc]
              const MAX: int  private @ 7:23-7:26  [doc]
              property $name: string  protected readonly @ 11:31-11:36  [doc]
              method make(): static  public static abstract @ 14:37-14:41  [doc]
        "#]],
    );
}

#[test]
fn enum_cases_carry_their_own_docblocks() {
    check_hir(
        r#"
//- /src/Suit.php
<?php
enum Suit
{
    /** Red. */
    case Hearts;
    case Spades;
}
"#,
        expect![[r#"
            enum Suit @ 2:6-2:10
              case Hearts  public @ 5:10-5:16  [doc]
              case Spades  public @ 6:10-6:16
        "#]],
    );
}
