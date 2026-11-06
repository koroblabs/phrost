//! `actions::promote`: a parameter and the property it fills, made one declaration.
//!
//! The *file the edits produce* is snapshotted, because three edits at three places either add up to
//! a class that says the same thing or they do not, and no one of them shows that alone.
//!
//! Most of these are refusals. Promotion changes what a class means when the three parts are not
//! equivalent, and nothing would fail until it ran -- so what is worth pinning is the shapes this
//! declines to touch.

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
            .find(|action| action.title.starts_with("Promote"))
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
        assert!(
            !text.lines().any(|line| line != line.trim_end()),
            "a line was left ending in whitespace:\n{text}"
        );
        format!("{}\n----\n{text}", action.title)
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn the_property_the_assignment_and_the_visibility_all_move_at_once() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
  private string $title;

  protected readonly int $count;

  public function __construct(string $ti@at@tle, int $count)
  {
    $this->title = $title;
    $this->count = $count;
  }
}
"#,
        expect![[r#"
            Promote $title into the constructor
            ----
            <?php
            namespace App;

            class Report
            {
              protected readonly int $count;

              public function __construct(private string $title, int $count)
              {
                $this->count = $count;
              }
            }
        "#]],
    );
}

#[test]
fn a_bare_var_docblock_goes_with_the_declaration_it_repeats() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
  /**
   * @var string
   */
  private $title;

  public function __construct($ti@at@tle)
  {
    $this->title = $title;
  }
}
"#,
        expect![[r#"
            Promote $title into the constructor
            ----
            <?php
            namespace App;

            class Report
            {
              public function __construct(private $title)
              {
              }
            }
        "#]],
    );
}

#[test]
fn a_docblock_that_says_something_is_not_this_actions_to_delete() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
  /**
   * The title, as the customer wrote it.
   */
  private string $title;

  public function __construct(string $ti@at@tle)
  {
    $this->title = $title;
  }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_type_the_property_spells_differently_is_refused() {
    // `?string` filled by a `string` parameter would become a `private string`, which is a narrower
    // property than the class declared -- and nothing would fail until something assigned null.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
  private ?string $title;

  public function __construct(string $ti@at@tle)
  {
    $this->title = $title;
  }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_body_that_does_more_than_assign_is_refused() {
    // `trim($title)` is not what promotion produces, and neither is a second write to the slot.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
  private string $title;

  public function __construct(string $ti@at@tle)
  {
    $this->title = trim($title);
  }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_variadic_parameter_cannot_be_promoted_and_neither_can_a_reference() {
    // PHP forbids both outright.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
  private array $rows;

  public function __construct(string ...$ro@at@ws)
  {
    $this->rows = $rows;
  }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_property_with_a_default_keeps_it_and_is_left_alone() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
  private string $title = 'none';

  public function __construct(string $ti@at@tle)
  {
    $this->title = $title;
  }
}
"#,
        expect!["(nothing offered)\n"],
    );
}
