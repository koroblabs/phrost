//! `actions::constructor`: the constructor a class does not have.
//!
//! The *file the edit produces* is snapshotted, because a constructor is right or wrong as a whole:
//! the parameter list, the assignments and the indentation are three things that each have to match
//! the class, and a snapshot of the text alone would show none of them in place.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

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
            .find(|action| action.title == "Generate a constructor")
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
        // The same property the generated stubs are held to: a whitespace-only line is what a
        // snapshot cannot show and what every PHP standard reports.
        assert!(
            !text.lines().any(|line| line != line.trim_end()),
            "a generated line ends in whitespace:\n{text}"
        );
        text
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn every_slot_the_class_declares_becomes_a_parameter_and_an_assignment() {
    // Each parameter spells its type the way the property does -- the two are written into one
    // file, so a short name already means there what it means above.
    check(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row {}

//- /src/Report.php
<?php
namespace App;

class Rep@at@ort
{
  private string $title;

  private ?Row $row;

  protected int $count;
}
"#,
        expect![[r#"
            <?php
            namespace App;

            class Report
            {
              private string $title;

              private ?Row $row;

              protected int $count;

              public function __construct(string $title, ?Row $row, int $count)
              {
                $this->title = $title;
                $this->row = $row;
                $this->count = $count;
              }
            }
        "#]],
    );
}

#[test]
fn a_slot_that_fills_itself_is_not_a_parameter() {
    // A property with a default is a memoised slot rather than something a caller supplies, and a
    // static one belongs to the class. Both are left where they are, and the constructor still goes
    // after the last of them -- which is where PHP is conventionally written.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Rep@at@ort
{
  private string $title;

  private ?string $cache = NULL;

  public static int $made = 0;
}
"#,
        expect![[r#"
            <?php
            namespace App;

            class Report
            {
              private string $title;

              private ?string $cache = NULL;

              public static int $made = 0;

              public function __construct(string $title)
              {
                $this->title = $title;
              }
            }
        "#]],
    );
}

#[test]
fn an_untyped_property_gives_its_parameter_no_type_either() {
    // Writing `mixed` would be a narrower claim than the class makes: an untyped property accepts
    // anything, and a `mixed` parameter still has to be passed.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Rep@at@ort
{
  private $anything;
}
"#,
        expect![[r#"
            <?php
            namespace App;

            class Report
            {
              private $anything;

              public function __construct($anything)
              {
                $this->anything = $anything;
              }
            }
        "#]],
    );
}

#[test]
fn a_class_that_has_one_is_offered_nothing() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Rep@at@ort
{
  private string $title;

  public function __construct(string $title)
  {
    $this->title = $title;
  }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_class_with_nothing_to_fill_is_offered_nothing() {
    // Every slot it has fills itself, so a constructor would take no arguments and do nothing.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Rep@at@ort
{
  private ?string $cache = NULL;

  public function run(): void {}
}
"#,
        expect!["(nothing offered)\n"],
    );
}
