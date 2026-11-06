//! What a string is known to hold, kept through lowering: the docblock refinements, and what a
//! string the source builds says by its own syntax.
//!
//! `non-empty-string` and `non-falsy-string` folded into `string` here, which left nothing for any
//! reader to know -- a parameter declaring one took a `''` without a word. Kept as builtins of their
//! own, and the forms that say more read as the non-empty one, the weaker claim that stays true.
//! `numeric-string` is deliberately not among them: see the lowering for why.

use expect_test::expect;

use super::{check_bodies, check_hir};

#[test]
fn a_docblock_refinement_of_string_is_kept() {
    check_hir(
        r#"
//- /src/Names.php
<?php
/**
 * @property non-empty-string $nonEmpty
 * @property non-falsy-string $nonFalsy
 * @property truthy-string $truthy
 * @property non-empty-lowercase-string $lowered
 * @property numeric-string $numeric
 * @property lowercase-string $lowercase
 * @property 'draft'|'sent' $status
 * @property '' $empty
 * @property '0' $zero
 * @property class-string $class
 */
class Names {}
"#,
        expect![[r#"
            class Names @ 14:7-14:12  [doc]
              property $nonEmpty: non-empty-string  public @ 3:31-3:40
              property $nonFalsy: non-falsy-string  public @ 4:31-4:40
              property $truthy: non-falsy-string  public @ 5:28-5:35
              property $lowered: non-empty-string  public @ 6:41-6:49
              property $numeric: string  public @ 7:29-7:37
              property $lowercase: string  public @ 8:31-8:41
              property $status: non-falsy-string|non-falsy-string  public @ 9:29-9:36
              property $empty: string  public @ 10:17-10:23
              property $zero: non-empty-string  public @ 11:18-11:23
              property $class: class-string  public @ 12:27-12:33
        "#]],
    );
}

#[test]
fn a_string_built_in_place_holds_what_its_written_text_does() {
    check_bodies(
        r#"
//- /src/Build.php
<?php
function build(string $s, int $i): void
{
    $a = $s . $s;
    $b = $s . '_id';
    $c = '0' . $s;
    $d = "{$s}";
    $e = "user:{$s}";
    $f = "0{$s}";
    $g = $s . $i;
    $s .= 'x';
    $s .= '';
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-14:1
                ?? declaration (arrival) @ 2:1-13:2
            function build
              body @ 3:1-13:2
                assign @ 4:5-4:17
                  $a @ 4:5-4:7
                  =
                    operator -> string @ 4:10-4:17
                      $s @ 4:10-4:12
                      $s @ 4:15-4:17
                assign @ 5:5-5:20
                  $b @ 5:5-5:7
                  =
                    operator -> non-falsy-string @ 5:10-5:20
                      $s @ 5:10-5:12
                      string @ 5:15-5:20
                assign @ 6:5-6:18
                  $c @ 6:5-6:7
                  =
                    operator -> non-empty-string @ 6:10-6:18
                      string @ 6:10-6:13
                      $s @ 6:16-6:18
                assign @ 7:5-7:16
                  $d @ 7:5-7:7
                  =
                    operator -> string @ 7:10-7:16
                      $s @ 7:12-7:14
                assign @ 8:5-8:21
                  $e @ 8:5-8:7
                  =
                    operator -> non-falsy-string @ 8:10-8:21
                      $s @ 8:17-8:19
                assign @ 9:5-9:17
                  $f @ 9:5-9:7
                  =
                    operator -> non-empty-string @ 9:10-9:17
                      $s @ 9:13-9:15
                assign @ 10:5-10:17
                  $g @ 10:5-10:7
                  =
                    operator -> string @ 10:10-10:17
                      $s @ 10:10-10:12
                      $i @ 10:15-10:17
                compound assign @ 11:5-11:14
                  $s @ 11:5-11:7
                    string @ 11:11-11:14
                compound assign @ 12:5-12:13
                  $s @ 12:5-12:7
                    string @ 12:11-12:13
        "#]],
    );
}
