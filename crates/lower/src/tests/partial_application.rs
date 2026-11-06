//! First-class callables: `strlen(...)`, `$repo->find(...)`, `Repo::find(...)`.
//!
//! Two things have to be true of this shape, and they are checked on two harnesses.
//!
//! The **name-reference table**, because PHP keeps three of them and every consumer picks one: goto
//! looks a class up among classes, an import is bound in the table its `use` belongs to,
//! find-references counts within a table. The shape used to fall through to the generic walk, which
//! saw a bare identifier and recorded it as a *constant* -- so all of them searched the wrong one.
//!
//! The **body**, because a class name resolving is only half of `Repo::find(...)`: without a node
//! carrying `find`, the method name is in no table at all and hover, goto, find-references and
//! rename answer nothing on it. What the body snapshot has to show is that the node is a `callable`
//! and not a `call` -- and that the receiver under it appears exactly once.

use expect_test::expect;

use super::{check_bodies, check_name_refs};

#[test]
fn a_static_methods_class_is_a_class_and_not_a_constant() {
    check_name_refs(
        r#"
//- /src/a.php
<?php

class Repo
{
    public static function find(): void {}
}

function go(): void
{
    $fn = Repo::find(...);
}
"#,
        expect![[r#"
            Repo  [ClassLike] @ 10:11-10:15
        "#]],
    );
}

#[test]
fn a_plain_functions_name_lands_in_the_function_table() {
    // Not the class-like table and not the constant one: `use function App\render;` binds here, and
    // looking a function up among classes finds a class that happens to share the name.
    check_name_refs(
        r#"
//- /src/a.php
<?php

function go(): void
{
    $fn = strlen(...);
}
"#,
        expect![[r#"
            strlen  [Function] @ 5:11-5:17
        "#]],
    );
}

#[test]
fn an_imported_class_is_seen_through_a_first_class_callable() {
    // The shape that made an import read as unused. The `use` itself lives in its own arena, so
    // what matters here is that the *usage* is recorded class-like -- that is the table the import
    // is bound in, and matching them is how the unused-import projection decides.
    check_name_refs(
        r#"
//- /src/a.php
<?php

namespace App;

use App\Data\Repo;

function go(): void
{
    $fn = Repo::find(...);
}
"#,
        expect![[r#"
            Repo  [ClassLike] @ 9:11-9:15
        "#]],
    );
}

#[test]
fn a_method_on_a_value_names_no_class() {
    // `$repo->find(...)` names the method on whatever the receiver holds, and the receiver is a
    // variable -- so there is no class-like name written here at all, and inventing one would be a
    // reference to something nobody wrote.
    check_name_refs(
        r#"
//- /src/a.php
<?php

function go($repo): void
{
    $fn = $repo->find(...);
}
"#,
        expect![[r#"
            <no name references>
        "#]],
    );
}

#[test]
fn a_relative_class_is_still_relative() {
    check_name_refs(
        r#"
//- /src/a.php
<?php

class Repo
{
    public static function find(): void {}

    public function make(): void
    {
        $fn = self::find(...);
    }
}
"#,
        expect![[r#"
            <no name references>
        "#]],
    );
}

#[test]
fn the_method_name_lands_in_the_body_as_a_callable_and_not_as_a_call() {
    // The whole body, so the receiver is visible exactly once. Turning an unmodelled shape into a
    // modelled one is how the body IR of every PHP file was once doubled with nothing failing, and a
    // whole-body snapshot is the only assertion that would have shown it.
    check_bodies(
        r#"
//- /src/a.php
<?php

class Repo
{
    public static function find(int $id): void {}

    public function one(): void {}
}

function go(Repo $repo): void
{
    $a = Repo::find(...);
    $b = $repo->one(...);
    $c = strlen(...);
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-16:1
                ?? declaration (arrival) @ 3:1-8:2
                ?? declaration (arrival) @ 10:1-15:2
            function go
              body @ 11:1-15:2
                assign @ 12:5-12:25
                  $a @ 12:5-12:7
                  =
                    callable Repo::find(...) @ 12:10-12:25 (name @ 12:16-12:20)
                assign @ 13:5-13:25
                  $b @ 13:5-13:7
                  =
                    callable ->one(...) @ 13:10-13:25 (name @ 13:17-13:20)
                      receiver
                        $repo @ 13:10-13:15
                assign @ 14:5-14:21
                  $c @ 14:5-14:7
                  =
                    callable strlen(...) @ 14:10-14:21
            Repo::find
              body @ 5:48-5:50
            Repo::one
              body @ 7:33-7:35
        "#]],
    );
}

#[test]
fn a_computed_name_keeps_the_variable_that_computes_it() {
    // `$repo->$name(...)` names no method, and the variable that decides which one is the whole of
    // what is written -- unreachable, it is a variable a rename would leave behind.
    check_bodies(
        r#"
//- /src/a.php
<?php

function go($repo, string $name): void
{
    $fn = $repo->$name(...);
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-7:1
                ?? declaration (arrival) @ 3:1-6:2
            function go
              body @ 4:1-6:2
                assign @ 5:5-5:28
                  $fn @ 5:5-5:8
                  =
                    callable -><computed>(...) @ 5:11-5:28 (name @ 5:18-5:23)
                      receiver
                        $repo @ 5:11-5:16
                      name
                        $name @ 5:18-5:23
        "#]],
    );
}

#[test]
fn a_partial_application_php_does_not_have_stays_unmodelled_and_keeps_its_arguments() {
    // mago parses the partial application an RFC proposed and no release adopted. Modelling it as a
    // callable would drop the arguments it binds, so it keeps the unmodelled treatment -- and
    // everything written inside it is still lowered.
    check_bodies(
        r#"
//- /src/a.php
<?php

function go($repo, int $id): void
{
    $fn = $repo->find($id, ?);
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-7:1
                ?? declaration (arrival) @ 3:1-6:2
            function go
              body @ 4:1-6:2
                assign @ 5:5-5:30
                  $fn @ 5:5-5:8
                  =
                    ?? expression @ 5:11-5:30
                      $repo @ 5:11-5:16
                      $id @ 5:23-5:26
        "#]],
    );
}
