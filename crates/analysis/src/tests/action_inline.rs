//! `actions::inline`: a variable's value put back where its name is used.
//!
//! The *file the edits produce* is snapshotted: the assignment going and every use being replaced
//! are one change, and either alone leaves a file that does not compile.
//!
//! Most of these are refusals, and the important one is not a syntax rule -- it is that inlining a
//! value read twice makes it *run* twice. Nothing about the file would say so afterwards.

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
            .find(|action| action.title.starts_with("Inline "))
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
        format!("{}\n----\n{text}", action.title)
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn a_value_read_once_goes_back_where_it_is_read() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(Report $other): void
    {
        $ti@at@tle = $other->name();
        echo $title;
    }

    public function name(): string
    {
        return '';
    }
}
"#,
        expect![[r#"
            Inline $title
            ----
            <?php
            namespace App;

            class Report
            {
                public function run(Report $other): void
                {
                    echo $other->name();
                }

                public function name(): string
                {
                    return '';
                }
            }
        "#]],
    );
}

#[test]
fn a_call_read_twice_would_run_twice_and_is_refused() {
    // The whole point of the rule, and nothing about the file would say so afterwards: two calls
    // where the author wrote one.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(Report $other): void
    {
        $ti@at@tle = $other->name();
        echo $title;
        echo $title;
    }

    public function name(): string
    {
        return '';
    }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_literal_costs_nothing_to_repeat() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(): void
    {
        $ti@at@tle = 'none';
        echo $title;
        echo $title;
    }
}
"#,
        expect![[r#"
            Inline $title
            ----
            <?php
            namespace App;

            class Report
            {
                public function run(): void
                {
                    echo 'none';
                    echo 'none';
                }
            }
        "#]],
    );
}

#[test]
fn a_name_written_twice_was_answering_which_value_a_use_meant() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(bool $flag): void
    {
        $ti@at@tle = 'none';
        if ($flag) {
            $title = 'some';
        }
        echo $title;
    }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_parameter_has_no_value_to_put_anywhere() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(string $ti@at@tle): void
    {
        echo $title;
    }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_name_nothing_reads_is_left_to_the_check_that_reports_it() {
    // `unused-variable` already says so, and removing the line is that finding's repair rather than
    // this one's -- inlining into nowhere is deleting code without saying it.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(Report $other): void
    {
        $ti@at@tle = $other->name();
    }

    public function name(): string
    {
        return '';
    }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

/// A name a compound assignment writes to, which is a second write and so not inlinable.
///
/// `$offset += $batch_size` puts a value in `$offset` as surely as `=` does, and the rule above --
/// one write, or the variable was answering which value a use meant -- already covers it. It did
/// not, because a compound assignment lowered to an unmodelled node and the target inside it read
/// as an ordinary use: the offer appeared, and accepting it rewrote a real project's script to
/// `0 += $batch_size;`, which PHP will not parse.
///
/// Found by `tools/code-actions-apply-cleanly.py`, which applies what every offer says and hands
/// the result to `php -l`.
#[test]
fn a_variable_a_compound_assignment_writes_to_is_not_offered() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

function run(int $size): int
{
    $offset = 0;
    while ($offset < 10) {
        echo $off@at@set;
        $offset += $size;
    }

    return $offset;
}
"#,
        expect![[r#"
            (nothing offered)
        "#]],
    );
}

/// The control for the one above: the same shape with a plain second write, which the rule already
/// refused before compound assignments were visible -- and a single-write version that is offered,
/// so the refusal is about the write and not about the loop.
#[test]
fn the_same_name_written_once_is_still_offered() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

function run(): string
{
    $prefix = 'a';

    return $pre@at@fix . 'b';
}
"#,
        expect![[r#"
            Inline $prefix
            ----
            <?php
            namespace App;

            function run(): string
            {

                return 'a' . 'b';
            }
        "#]],
    );
}

#[test]
fn a_variable_compact_collects_by_name_is_not_offered() {
    // `compact('title')` reads the variable by its name. There is no value to write inside the
    // quotes, and removing the assignment leaves the call collecting nothing.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

function render(): array
{
    $ti@at@tle = 'Report';
    return compact('title');
}
"#,
        expect![[r#"
            (nothing offered)
        "#]],
    );
}

/// A name read inside a string is interpolated, and only a variable, a property or an element may
/// stand there -- the value put in its place made a file PHP will not parse.
#[test]
fn a_variable_a_string_interpolates_is_not_offered() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

function report(array $errors): string
{
    $mess@at@age = implode(', ', $errors);
    return "Invalid: $message";
}
"#,
        expect![[r#"
            (nothing offered)
        "#]],
    );
}

/// The braced spelling, `{$message}`, is interpolation all the same.
#[test]
fn a_variable_a_string_interpolates_in_braces_is_not_offered() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

function report(array $errors): string
{
    $mess@at@age = implode(', ', $errors);
    return "Invalid: {$message}!";
}
"#,
        expect![[r#"
            (nothing offered)
        "#]],
    );
}

/// `$this->$option_tag` reads the variable's value as a property's name, and only another variable
/// may stand there.
#[test]
fn a_variable_read_as_a_members_name_is_not_offered() {
    check(
        r#"
//- /src/Options.php
<?php
namespace App;

class Options
{
    public bool $option_tag_id3 = true;

    public function on(string $tag): bool
    {
        $option@at@_tag = 'option_tag_' . $tag;
        return $this->$option_tag;
    }
}
"#,
        expect![[r#"
            (nothing offered)
        "#]],
    );
}
