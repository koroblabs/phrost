//! `/** @var T $x */` above a statement mago's error recovery dropped.
//!
//! The annotation is normally emitted by asking the statement below the docblock for it, which
//! works right up until there is no statement: at the end of a file mago has no closing token to
//! resynchronize on, so it drops what it could not finish rather than keeping a partial node. What
//! it drops is `$x->` with the member still unwritten -- the statement someone is typing, and the
//! one moment the `@var` above it is the only thing that says what `$x` is.
//!
//! Since the parse repair, the *statement* usually survives too: `ParseSession::parse` supplies the
//! terminator mago says is missing and re-parses, so the snapshots below show a `property ->` where
//! they used to show nothing. That does not make the window here redundant -- it is what the
//! annotation is read from when the repair cannot finish the tail, and it is why the two are checked
//! together.

use expect_test::expect;

use super::check_bodies;

#[test]
fn an_annotation_survives_a_statement_recovery_dropped() {
    check_bodies(
        r#"
//- /src/C.php
<?php

class Foo
{
    public function fooMethod(): void {}
}

/** @var Foo[] $foo */
$foo[0]->
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 3:1-6:2
                @var $foo Foo[]
                property -><computed> @ 9:1-9:10
                  receiver
                    index @ 9:1-9:8
                      $foo @ 9:1-9:5
                      int @ 9:6-9:7
            Foo::fooMethod
              body @ 5:39-5:41
            !! Expected one of `Semicolon`, `CloseTag` before end of file
        "#]],
    );
}

/// The same for a namespaced file, where the dropped statement leaves the `namespace` node ending
/// at its own semicolon and the docblock outside every node the tree kept.
#[test]
fn a_namespaced_file_keeps_the_orphaned_annotation() {
    check_bodies(
        r#"
//- /src/C.php
<?php
namespace App;

/** @var Foo $foo */
$foo->
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-6:1
                block @ 2:1-6:1
                  @var $foo Foo
                  property -><computed> @ 5:1-5:7
                    receiver
                      $foo @ 5:1-5:5
            !! Expected one of `Semicolon`, `CloseTag` before end of file
        "#]],
    );
}

/// A method body ends at its `}`, so the recovery reads the span after its last statement and not
/// the file's -- one annotation, in the scope it was written in.
#[test]
fn a_docblock_at_the_end_of_a_method_stays_in_that_method() {
    check_bodies(
        r#"
//- /src/C.php
<?php
class Hook
{
    public function run(): void
    {
        $a = 1;
        /** @var Special $entity */
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 2:1-9:2
            Hook::run
              body @ 5:5-8:6
                assign @ 6:9-6:15
                  $a @ 6:9-6:11
                  =
                    int @ 6:14-6:15
                @var $entity Special
        "#]],
    );
}

/// The statement below the docblock survived, so it asked for the annotation itself. Nothing is
/// left over for the recovery to read, and the whole-body snapshot is what says so: a second
/// `@var $entity Special` would be invisible to any assertion that only looked for one.
#[test]
fn a_surviving_statement_does_not_get_a_second_annotation() {
    check_bodies(
        r#"
//- /src/C.php
<?php
class Hook
{
    public function run(): void
    {
        /** @var Special $entity */
        $entity;
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 2:1-9:2
            Hook::run
              body @ 5:5-8:6
                @var $entity Special
                $entity @ 7:9-7:16
        "#]],
    );
}

/// mago merges an unfinished statement into the one after it rather than dropping it when there is
/// something to merge with, so the docblock still has a statement below it. Recorded because it is
/// the case that makes the recovery look unnecessary until the dropped one is tried.
#[test]
fn a_statement_recovery_merged_forward_needs_no_recovery() {
    check_bodies(
        r#"
//- /src/C.php
<?php
/** @var Foo $foo */
$foo->
$other = 1;
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-5:1
                @var $foo Foo
                assign @ 3:1-4:11
                  property -><computed> @ 3:1-4:7
                    receiver
                      $foo @ 3:1-3:5
                  =
                    int @ 4:10-4:11
        "#]],
    );
}

/// A nameless `@var` binds nothing on its own -- it describes whatever is written below it, and
/// with the statement gone there is nothing for it to describe. Inventing a binding from the tag's
/// type alone would put a variable in scope that the author never named.
#[test]
fn a_nameless_var_is_not_recovered() {
    check_bodies(
        r#"
//- /src/C.php
<?php
/** @var Foo */
$foo->
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-4:1
                property -><computed> @ 3:1-3:7
                  receiver
                    $foo @ 3:1-3:5
            !! Expected one of `Semicolon`, `CloseTag` before end of file
        "#]],
    );
}
