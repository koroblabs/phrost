//! Writing the docblock a declaration does not have.
//!
//! The whole text is snapshotted, not the line it starts on: the layout *is* the feature, and a test
//! that showed only `/**` would pass whatever came after it.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

/// The docblock the offer would insert, with the file's own indentation shown.
fn check(fixture: &str, marker: &str, style: Option<&str>, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    if let Some(style) = style {
        world.set_house_style(style);
    }
    let world = world;
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let at = phrost_model::TextRange::empty(position.offset);
        let request = crate::ActionRequest {
            view: &view,
            at,
            only: &[],
            asked: Default::default(),
        };
        let Some(action) = code_actions(&request)
            .into_iter()
            .find(|action| action.title.starts_with("Write a docblock"))
        else {
            return "(nothing to write)\n".to_owned();
        };
        // The inserted text, with the trailing indentation it re-emits shown as `<>` so a snapshot
        // that lost it would be visible: the declaration has to end up back where it started.
        action
            .edits
            .iter()
            .map(|edit| edit.text.replace("    ", "····"))
            .collect::<String>()
            + "<>\n"
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn drupal_puts_each_description_on_its_own_indented_line() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Thing {}

class Repo
{
    public function fi@here@nd(int $id, ?Thing $fallback = NULL): ?Thing
    {
        return $fallback;
    }
}
"#,
        "here",
        Some("drupal"),
        expect![[r#"
            /**
            ···· * Finds.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· * @param ?\App\Thing $fallback
            ···· *   The fallback.
            ···· *
            ···· * @return ?\App\Thing
            ···· *   The thing.
            ···· */
            ····<>
        "#]],
    );
}

#[test]
fn the_plain_layout_puts_it_beside_the_tag() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function co@here@unt(string $of): int
    {
        return 0;
    }
}
"#,
        "here",
        None,
        expect![[r#"
            /**
            ···· * Counts.
            ···· *
            ···· * @param string $of The of.
            ···· * @return int The int.
            ···· */
            ····<>
        "#]],
    );
}

#[test]
fn a_method_that_returns_nothing_gets_no_return_tag() {
    // `@return void` repeats the signature and says nothing, which is a line somebody deletes.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function re@here@set(): void {}
}
"#,
        "here",
        Some("drupal"),
        expect![[r#"
            /**
            ···· * Resets.
            ···· */
            ····<>
        "#]],
    );
}

#[test]
fn a_declaration_that_already_has_one_is_left_alone() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Resets it.
     */
    public function re@here@set(): void {}
}
"#,
        "here",
        Some("drupal"),
        expect!["(nothing to write)\n"],
    );
}

/// What the body throws is written below the return, the same way every other tag group is.
#[test]
fn what_the_body_throws_gets_a_throws_group() {
    check(
        r#"
//- /src/spl.php
<?php
class Exception {}
class InvalidArgumentException extends Exception {}

//- /src/Repo.php
<?php
namespace App;

class Missing extends \Exception {}

class Repo
{
    public function fi@here@nd(int $id): int
    {
        if ($id < 0) {
            throw new \InvalidArgumentException('negative');
        }
        if ($id === 0) {
            throw new Missing('none');
        }
        if ($id > 100) {
            throw new Missing('too many');
        }
        return $id;
    }
}
"#,
        "here",
        Some("drupal"),
        expect![[r#"
            /**
            ···· * Finds.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· *
            ···· * @return int
            ···· *   The int.
            ···· *
            ···· * @throws \InvalidArgumentException
            ···· *   Thrown when an argument is invalid.
            ···· * @throws \App\Missing
            ···· *   Thrown when the operation fails.
            ···· */
            ····<>
        "#]],
    );
}

/// A free function gets a docblock the way a method does. A Drupal `.module` file is made of
/// them, and this was the one shape the offer was silent on while the rule reporting the gap fired.
#[test]
fn a_free_function_gets_a_docblock_too() {
    check(
        r#"
//- /src/mymodule.php
<?php

class Missing extends \Exception {}

/**
 * Implements hook_help().
 */
function mymodule_help(string $route): string {
  return $route;
}

function mymodule_bu@here@ild(array $items, int $limit): array {
  if ($limit < 0) {
    throw new Missing('negative');
  }
  return array_slice($items, 0, $limit);
}
"#,
        "here",
        Some("drupal"),
        expect![[r#"
            /**
             * Mymodules build.
             *
             * @param array $items
             *   The items.
             * @param int $limit
             *   The limit.
             *
             * @return array
             *   The array.
             *
             * @throws \Missing
             *   Thrown when the operation fails.
             */
            <>
        "#]],
    );
}

/// The keys the body writes out, in the tag that would otherwise say `array` twice over.
///
/// The one thing the generator still cannot supply is what the array *is*, so the description
/// stays the line to overwrite -- everything a reader could have derived is now in the type.
#[test]
fn a_keyed_return_is_written_out_key_by_key() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class File
{
    public static function load(int $id): ?File
    {
        return NULL;
    }
}

class Repo
{
    public function getStore@here@Sales(int $id): ?array
    {
        if ($id < 0) {
            return NULL;
        }
        return [
            'date' => 'today',
            'rate' => (float) $id,
            'amount' => $id,
            'file' => File::load($id),
        ];
    }
}
"#,
        "here",
        Some("drupal"),
        expect![[r#"
            /**
            ···· * Gets the store sales.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· *
            ···· * @return ?array{
            ···· *   date: non-falsy-string,
            ···· *   rate: float,
            ···· *   amount: int,
            ···· *   file: ?\App\File,
            ···· * }
            ···· *   The array.
            ···· */
            ····<>
        "#]],
    );
}

/// A shape too long for one line goes one field to a line, and the description moves under the
/// closing brace -- after a `}` on the same line it reads as part of the type rather than as prose
/// about it. Both layouts do it, because at this width neither has a one-line form left.
#[test]
fn a_shape_too_long_for_its_line_is_written_one_field_to_a_line() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App\Reporting;

class Attachment {}

class Repo
{
    public function retail@here@Snapshot(int $id): ?array
    {
        return [
            'reference' => 'x',
            'rate_of_return' => (float) $id,
            'amount_in_cents' => $id,
            'attachment' => new Attachment(),
        ];
    }
}
"#,
        "here",
        Some("drupal"),
        expect![[r#"
            /**
            ···· * Retails snapshot.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· *
            ···· * @return ?array{
            ···· *   reference: non-falsy-string,
            ···· *   rate_of_return: float,
            ···· *   amount_in_cents: int,
            ···· *   attachment: \App\Reporting\Attachment,
            ···· * }
            ···· *   The array.
            ···· */
            ····<>
        "#]],
    );
}
