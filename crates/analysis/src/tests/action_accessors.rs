//! `actions::accessors`: the getters and setters a class's properties do not have.
//!
//! The *file the edits produce* is snapshotted, and both offers are shown at once: they are two
//! decisions rather than one, and what makes them right is that the same class gets a different
//! answer to each.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

/// Every accessor offer, its title and the file it would leave behind.
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
        let offered: Vec<_> = code_actions(&request)
            .into_iter()
            .filter(|action| action.title.contains("getter") || action.title.contains("setter"))
            .collect();
        if offered.is_empty() {
            return "(nothing offered)\n".to_owned();
        }

        let mut out = String::new();
        for action in offered {
            let mut text = world.text_of(file).to_owned();
            let mut edits = action.edits.clone();
            edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
            for edit in edits {
                let range = usize::from(edit.range.start())..usize::from(edit.range.end());
                text.replace_range(range, &edit.text);
            }
            assert!(
                !text.lines().any(|line| line != line.trim_end()),
                "a generated line ends in whitespace:\n{text}"
            );
            out.push_str(&format!("{}\n----\n{text}\n====\n", action.title));
        }
        out
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn a_bool_is_asked_rather_than_got_and_a_readonly_slot_is_never_written() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Rep@at@ort
{
  private string $title;

  private bool $active;

  private bool $isDraft;

  public readonly int $count;
}
"#,
        expect![[r#"
            Generate getters for 4 properties
            ----
            <?php
            namespace App;

            class Report
            {
              private string $title;

              private bool $active;

              private bool $isDraft;

              public readonly int $count;

              public function getTitle(): string
              {
                return $this->title;
              }

              public function isActive(): bool
              {
                return $this->active;
              }

              public function isDraft(): bool
              {
                return $this->isDraft;
              }

              public function getCount(): int
              {
                return $this->count;
              }
            }

            ====
            Generate setters for 3 properties
            ----
            <?php
            namespace App;

            class Report
            {
              private string $title;

              private bool $active;

              private bool $isDraft;

              public readonly int $count;

              public function setTitle(string $title): void
              {
                $this->title = $title;
              }

              public function setActive(bool $active): void
              {
                $this->active = $active;
              }

              public function setIsDraft(bool $isDraft): void
              {
                $this->isDraft = $isDraft;
              }
            }

            ====
        "#]],
    );
}

#[test]
fn an_accessor_the_class_already_declares_is_not_offered_again() {
    // A second `getTitle` is a parse error, and the count in the title is what will be written.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Rep@at@ort
{
  private string $title;

  private int $count;

  public function getTitle(): string
  {
    return $this->title;
  }

  public function setCount(int $count): void
  {
    $this->count = $count;
  }
}
"#,
        expect![[r#"
            Generate a getter for $count
            ----
            <?php
            namespace App;

            class Report
            {
              private string $title;

              private int $count;

              public function getTitle(): string
              {
                return $this->title;
              }

              public function setCount(int $count): void
              {
                $this->count = $count;
              }

              public function getCount(): int
              {
                return $this->count;
              }
            }

            ====
            Generate a setter for $title
            ----
            <?php
            namespace App;

            class Report
            {
              private string $title;

              private int $count;

              public function getTitle(): string
              {
                return $this->title;
              }

              public function setCount(int $count): void
              {
                $this->count = $count;
              }

              public function setTitle(string $title): void
              {
                $this->title = $title;
              }
            }

            ====
        "#]],
    );
}

#[test]
fn a_slot_no_accessor_can_reach_is_left_alone() {
    // A static property belongs to the class, and a hooked one already declares its own `get`.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Rep@at@ort
{
  public static int $made = 0;

  public int $doubled { get => 2; }
}
"#,
        expect!["(nothing offered)\n"],
    );
}
