//! `class_exists('App\Repo')` -- the way PHP named a class before `::class`.
//!
//! Three features were dead at such a site and none of them had a reason to be: goto answered
//! nothing, hover answered nothing, and a rename of the class moved every other spelling and left
//! this one naming a class that no longer exists. A Drupal annotation, a container definition keyed
//! by a class name, and an `is_a($x, 'App\Repo')` guard are all this shape.
//!
//! What keeps it from claiming every string in a project is that the name has to *be* a class the
//! index holds: a string is data, and nearly none of them resolve. Read as fully qualified, which
//! is what PHP does -- a string is not affected by the file's `use` statements, so `'Repo'` inside
//! `namespace App;` names the global one.
//!
//! The rename replaces the **last segment** and leaves the namespace in front of it: handing back
//! the whole path turns `'App\Repo'` into `'Store'`, a class in the global namespace, which is not
//! where it lives.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{references_in, rename_span, rename_target};

/// Every range a rename from the marked cursor would edit, across the fixture's files.
fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let mut out = String::new();

    let position = spec.markers().position(marker);
    let cursor = world.file_id(&position.path);
    let open = world.open(OpenScope::EveryFile, Some(cursor));
    let target = rename_target(
        &IndexView::in_file(world.skeleton(), &open, cursor),
        position.offset,
    );
    match target {
        None => out.push_str("nothing to rename\n"),
        Some(target) => {
            for file in spec.files() {
                let id = world.file_id(file.path());
                let view = IndexView::in_file(world.skeleton(), &open, id);
                for range in references_in(&view, &target) {
                    let edited = crate::rename_range(world.text_of(id), range);
                    let _ = writeln!(out, "{} {}", file.path(), world.render_range(id, edited));
                }
            }
        }
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

#[test]
fn a_rename_reaches_the_string_and_moves_its_last_segment() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class R@here@epo {}

//- /src/Client.php
<?php
namespace App;

class Client
{
    public function has(): bool
    {
        return class_exists('App\Repo') && is_a($this, Repo::class);
    }

    public function named(): array
    {
        return ['class' => 'App\Repo'];
    }
}
"#,
        "here",
        expect![[r#"
            src/Repo.php 4:7-4:11
            src/Client.php 8:34-8:38
            src/Client.php 8:56-8:60
            src/Client.php 13:33-13:37
        "#]],
    );
}

/// A string that names nothing the index holds is data, and stays data.
#[test]
fn a_string_that_names_no_class_is_not_a_reference() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class R@here@epo {}

//- /src/Client.php
<?php
namespace App;

class Client
{
    public function label(): string
    {
        return 'App\Repository';
    }
}
"#,
        "here",
        expect![[r#"
            src/Repo.php 4:7-4:11
        "#]],
    );
}

/// Where the dialogue is allowed to open, which must be exactly where the rename edits.
///
/// `prepareRename` used to build its own list of candidate ranges -- the file's name references,
/// its member accesses, and the declaration under the cursor -- and a class named by a string is in
/// none of the three. So the editor offered nothing at `is_a($x, 'App\Repo')` while a rename from
/// that very position edited forty sites in eighteen files on one real checkout. One rule, two
/// constructors, and only one of them had learned about strings.
fn check_span(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);

    let position = spec.markers().position(marker);
    let cursor = world.file_id(&position.path);
    let open = world.open(OpenScope::EveryFile, Some(cursor));
    let view = IndexView::in_file(world.skeleton(), &open, cursor);
    let out = match rename_span(&view, position.offset) {
        Some(range) => format!(
            "{} {}\n",
            position.path,
            world.render_range(cursor, crate::rename_range(world.text_of(cursor), range))
        ),
        None => "the dialogue does not open\n".to_owned(),
    };

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

#[test]
fn the_dialogue_opens_on_the_last_segment_of_a_string_that_names_a_class() {
    check_span(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo {}

//- /src/Client.php
<?php
namespace App;

class Client
{
    public function has(): bool
    {
        return is_a($this, 'App\R@here@epo');
    }
}
"#,
        "here",
        expect![[r#"
            src/Client.php 8:33-8:37
        "#]],
    );
}

#[test]
fn the_dialogue_does_not_open_on_a_string_that_names_nothing() {
    check_span(
        r#"
//- /src/Client.php
<?php
namespace App;

class Client
{
    public function label(): string
    {
        return 'App\Repos@here@itory';
    }
}
"#,
        "here",
        expect![[r#"
            the dialogue does not open
        "#]],
    );
}

/// A bare word names a global class only in the class's own case, at a cursor as in a rename.
///
/// The index folds case, as PHP does for a class, so `'name' => 'Received'` used to be a reference
/// to whatever global `Name` anything declared -- on a real checkout an enum in PHP_CodeSniffer's
/// test data -- and goto and hover on an array key went there.
#[test]
fn a_bare_word_in_another_case_is_not_the_global_class() {
    crate::tests::check(
        r#"
//- /src/Name.php
<?php
enum Name {}

//- /src/Status.php
<?php
namespace App;

class Status
{
    public const array ALL = [
        ['na@key@me' => 'Received'],
        ['class' => 'Na@exact@me'],
    ];
}
"#,
        &[
            crate::tests::Query::goto("lower-case key", "key"),
            crate::tests::Query::hover("lower-case key", "key"),
            crate::tests::Query::goto("the class's own case", "exact"),
        ],
        expect![[r#"
            lower-case key
            - (nothing to go to)
            lower-case key
            - (nothing to describe)
            the class's own case
            - enum Name @ src/Name.php 2:6-2:10 (full 2:1-2:13)
        "#]],
    );
}

/// The same word in the class's own case, when the class is a dependency's global one-word name:
/// on the reference project `enum Name` in PHP_CodeSniffer's test data, and every `'Name'` the
/// project wrote -- a key, a label, `$this->t('Name')`, `$fields['Name']` -- went there. A
/// dependency names its classes in a namespace; a global one is legacy or a fixture, and a word
/// that reads as one is a word. What this exists for stays: a compound name nobody writes by
/// accident, a class of the language's own, and one the project declares.
#[test]
fn a_dependencys_global_one_word_class_is_not_named_by_the_word() {
    crate::tests::check(
        r#"
//- /stubs/core.php  stub
<?php
class Exception {}

//- /vendor/acme/tests/Fixture.php
<?php
enum Name {}

class WP_Query {}

//- /src/Status.php
<?php
namespace App;

class Status
{
    public function run(): void
    {
        $label = ['Na@word@me' => $this->t('Na@label@me')];
        class_exists('WP_Q@compound@uery');
        is_a($this, 'Excep@language@tion');
    }
}
"#,
        &[
            crate::tests::Query::goto("a key", "word"),
            crate::tests::Query::hover("a label", "label"),
            crate::tests::Query::goto("a compound name", "compound"),
            crate::tests::Query::goto("the language's own", "language"),
        ],
        expect![[r#"
            a key
            - (nothing to go to)
            a label
            - (nothing to describe)
            a compound name
            - class WP_Query @ vendor/acme/tests/Fixture.php 4:7-4:15 (full 4:1-4:18)
            the language's own
            - class Exception @ stubs/core.php 2:7-2:16 (full 2:1-2:19)
        "#]],
    );
}

/// A function a string names is renamed from there too -- every call and the string itself -- so
/// the dialogue has to open on it. Laravel guards each of its helpers with
/// `function_exists('Illuminate\Support\enum_value')`, and the rename from that string edited 267
/// sites while `prepareRename` answered nothing.
#[test]
fn the_dialogue_opens_on_a_string_that_names_a_function() {
    check_span(
        r#"
//- /src/functions.php
<?php
namespace App\Support;

if (! function_exists('App\Support\enum_val@here@ue')) {
    function enum_value(mixed $value): mixed
    {
        return $value;
    }
}
"#,
        "here",
        expect![[r#"
            src/functions.php 4:36-4:46
        "#]],
    );
}
