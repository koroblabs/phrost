//! Replacing a `@return array` somebody already wrote with what the body hands back.
//!
//! The whole file after the edit, not the edit: what matters is that the description stays where
//! its author put it and the block still parses, and an edit shown as a range and a string proves
//! neither.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{ActionKind, code_actions};

/// The file with the offer applied, or the file unchanged when nothing is offered.
fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    world.set_house_style("drupal");
    let world = world;
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let request = crate::ActionRequest {
            view: &view,
            at: phrost_model::TextRange::empty(position.offset),
            only: &[ActionKind::Refactor],
            asked: Default::default(),
        };
        let Some(action) = code_actions(&request)
            .into_iter()
            .find(|action| action.title == "Say what the returned array holds")
        else {
            return "(nothing offered)\n".to_owned();
        };
        let mut text = world.text_of(file).to_string();
        // Back to front, so an earlier edit does not move a later one.
        let mut edits = action.edits.clone();
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
        for edit in edits {
            let range = usize::from(edit.range.start())..usize::from(edit.range.end());
            text.replace_range(range, &edit.text);
        }
        text
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn the_type_is_replaced_and_the_description_stays_where_its_author_put_it() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Returns retail sales info.
     *
     * @return ?array
     *   Retail sales info.
     */
    public function retail@here@Sales(int $id): ?array
    {
        if ($id < 0) {
            return NULL;
        }
        return [
            'date' => 'today',
            'rate' => (float) $id,
        ];
    }
}
"#,
        "here",
        expect![[r#"
            <?php
            namespace App;

            class Repo
            {
                /**
                 * Returns retail sales info.
                 *
                 * @return ?array{date: non-falsy-string, rate: float}
                 *   Retail sales info.
                 */
                public function retailSales(int $id): ?array
                {
                    if ($id < 0) {
                        return NULL;
                    }
                    return [
                        'date' => 'today',
                        'rate' => (float) $id,
                    ];
                }
            }
        "#]],
    );
}

/// A long one wraps in place, and the author's description follows the closing brace.
#[test]
fn a_long_one_is_written_one_field_to_a_line() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App\Reporting;

class Attachment {}

class Repo
{
    /**
     * @return array
     *   The snapshot.
     */
    public function retail@here@Snapshot(int $id): array
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
        expect![[r#"
            <?php
            namespace App\Reporting;

            class Attachment {}

            class Repo
            {
                /**
                 * @return array{
                 *   reference: non-falsy-string,
                 *   rate_of_return: float,
                 *   amount_in_cents: int,
                 *   attachment: \App\Reporting\Attachment,
                 * }
                 *   The snapshot.
                 */
                public function retailSnapshot(int $id): array
                {
                    return [
                        'reference' => 'x',
                        'rate_of_return' => (float) $id,
                        'amount_in_cents' => $id,
                        'attachment' => new Attachment(),
                    ];
                }
            }
        "#]],
    );
}

/// A precise tag beside the plain one is the author's answer, and better than this one: §8 records
/// that the plain `@return object|null` is written for a reader that cannot hold `?T`, so it is not
/// a second opinion to be corrected.
#[test]
fn a_prefixed_return_beside_the_plain_one_is_left_alone() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * @return array
     *   The rows.
     * @phpstan-return array{id: int}
     */
    public function ro@here@ws(): array
    {
        return ['id' => 1];
    }
}
"#,
        "here",
        expect![[r#"
            (nothing offered)
        "#]],
    );
}

/// A declaration whose tag already says something is not corrected, and neither is one whose body
/// says nothing this can read.
#[test]
fn nothing_is_offered_where_the_tag_or_the_body_already_settles_it() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * @return array<int, string>
     *   The rows.
     */
    public function ro@here@ws(): array
    {
        return ['id' => 'x'];
    }
}
"#,
        "here",
        expect![[r#"
            (nothing offered)
        "#]],
    );
}
