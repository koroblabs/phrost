//! `actions::implement`: writing the methods a class does not declare and cannot load without.
//!
//! The *file the edit produces* is snapshotted, because three things have to be right at once and
//! only the result shows all three: what is written, where it goes, and what it is indented by. A
//! stub that is correct PHP in the wrong place is a file that does not parse.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

/// The fixture's last file after the offered action is applied, and the title that offered it.
fn check(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position("at");
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let request = crate::ActionRequest {
            view: &view,
            at: phrost_model::TextRange::empty(position.offset),
            only: &[],
            asked: Default::default(),
        };
        let Some(action) = code_actions(&request)
            .into_iter()
            .find(|action| action.title.starts_with("Implement"))
        else {
            return "(nothing offered)\n".to_owned();
        };

        let mut text = world.text_of(file).to_owned();
        let mut edits = action.edits.clone();
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
        for edit in edits {
            let range = usize::from(edit.range.start())..usize::from(edit.range.end());
            text.replace_range(range, &edit.text);
        }
        // A property rather than a snapshot line, because a whitespace-only line is exactly what a
        // snapshot cannot show: expect-test writes it back at whatever indentation fits, and every
        // PHP standard there is reports one.
        assert!(
            !text.lines().any(|line| line != line.trim_end()),
            "a generated line ends in whitespace:\n{text}"
        );
        format!("{}\n----\n{text}", action.title)
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn every_method_an_interface_requires_is_written_into_the_body() {
    // After the last member, at the indentation the body already uses -- two spaces here, which is
    // what a Drupal file writes and what guessing from the language would have got wrong.
    check(
        r#"
//- /src/Writes.php
<?php
namespace App;

interface Writes
{
    public function open(string $path): bool;

    public function close(): void;
}

//- /src/File.php
<?php
namespace App;

class Fi@at@le implements Writes
{
  public const MODE = 'w';
}
"#,
        expect![[r#"
            Implement the 2 methods File requires
            ----
            <?php
            namespace App;

            class File implements Writes
            {
              public const MODE = 'w';

              public function close(): void
              {
      
              }

              public function open(string $path): bool
              {
      
              }
            }
        "#]],
    );
}

#[test]
fn an_empty_body_is_filled_just_inside_its_brace() {
    // The one position a body with nothing in it has, and the brace is found from the text: between
    // a class's name and its body PHP writes only `extends`, `implements` and the names they take.
    check(
        r#"
//- /src/Writes.php
<?php
namespace App;

interface Writes
{
    public function close(): void;
}

//- /src/File.php
<?php
namespace App;

class Fi@at@le implements Writes {}
"#,
        expect![[r#"
            Implement the 1 method File requires
            ----
            <?php
            namespace App;

            class File implements Writes {

                public function close(): void
                {

                }
            }
        "#]],
    );
}

/// A parameter's default is written back with the rest of the declaration.
///
/// The index used to keep that there *was* one rather than what it was, so a stub would have made
/// an optional parameter required and the class still would not have loaded -- and the repair
/// skipped the method instead, saying so in its title. That is **21 % of the reference project's
/// own interfaces and 30 % of Drupal core's**: a quarter of every use of this action stopped short.
///
/// The value is kept now, 25 029 of a checkout's 148 917 parameters carrying one at 0.1 MB of text.
/// What still cannot be written back is a *promoted* parameter, which declares a property too.
#[test]
fn a_parameters_default_is_written_back_with_it() {
    check(
        r#"
//- /src/Writes.php
<?php
namespace App;

interface Writes
{
    public function open(string $path, int $flags = 0): bool;

    public function close(): void;
}

//- /src/File.php
<?php
namespace App;

class Fi@at@le implements Writes
{
}
"#,
        expect![[r#"
            Implement the 2 methods File requires
            ----
            <?php
            namespace App;

            class File implements Writes
            {

                public function close(): void
                {

                }

                public function open(string $path, int $flags = 0): bool
                {

                }
            }
        "#]],
    );
}

#[test]
fn a_class_that_declares_everything_is_offered_nothing() {
    check(
        r#"
//- /src/Writes.php
<?php
namespace App;

interface Writes
{
    public function close(): void;
}

//- /src/File.php
<?php
namespace App;

class Fi@at@le implements Writes
{
    public function close(): void {}
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn an_abstract_class_is_allowed_to_leave_them() {
    // It does not load anything by itself, so nothing is broken and there is nothing to repair --
    // which is `unimplemented_methods`'s judgement rather than this offer's.
    check(
        r#"
//- /src/Writes.php
<?php
namespace App;

interface Writes
{
    public function close(): void;
}

//- /src/File.php
<?php
namespace App;

abstract class Fi@at@le implements Writes
{
}
"#,
        expect!["(nothing offered)\n"],
    );
}
