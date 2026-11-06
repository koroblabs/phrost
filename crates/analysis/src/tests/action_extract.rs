//! `actions::extract`: an expression lifted into a variable of its own.
//!
//! The *file the edits produce* is snapshotted, because the two edits are only right together: the
//! assignment above the statement and the name in place of what was selected have to describe the
//! same expression, at the same indentation, with a name nothing else is using.
//!
//! Most of these are refusals, and each is a way hoisting would change the program rather than the
//! text. That is the whole risk of this refactor: nothing fails, the code just runs when it did not.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_model::TextRange;
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

/// The file after the extraction offered over `from`..`to`, or over the cursor when only one marker
/// is written.
fn check(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let markers = spec.markers();
    let from = markers.position("from");
    // Both ends always, so a bare cursor is written as the two together and the harness needs no
    // branch: what an editor sends for a caret is an empty range at that point.
    let selected = TextRange::new(from.offset, markers.position("to").offset);
    let file = world.file_id(&from.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let request = crate::ActionRequest {
            view: &view,
            at: selected,
            only: &[],
            asked: Default::default(),
        };
        let Some(action) = code_actions(&request)
            .into_iter()
            .find(|action| action.title.starts_with("Extract "))
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

    markers.assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn a_call_is_named_after_the_method_with_its_verb_taken_off() {
    // `getTitle()` names a title, not a get -- and the assignment lands at the statement's own
    // indentation, which is two levels in here.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function title(): string
    {
        return '';
    }

    public function run(Report $other): void
    {
        if (true) {
            echo @from@$other->getTitle()@to@;
        }
    }

    public function getTitle(): string
    {
        return '';
    }
}
"#,
        expect![[r#"
            Extract $title
            ----
            <?php
            namespace App;

            class Report
            {
                public function title(): string
                {
                    return '';
                }

                public function run(Report $other): void
                {
                    if (true) {
                        $title = $other->getTitle();
                        echo $title;
                    }
                }

                public function getTitle(): string
                {
                    return '';
                }
            }
        "#]],
    );
}

#[test]
fn a_cursor_takes_the_innermost_thing_worth_a_name() {
    // On `$other` there is a variable, which already is one -- so the call around it is what a
    // person with the caret there meant.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(Report $other): void
    {
        echo $ot@from@@to@her->getTitle();
    }

    public function getTitle(): string
    {
        return '';
    }
}
"#,
        expect![[r#"
            Extract $title
            ----
            <?php
            namespace App;

            class Report
            {
                public function run(Report $other): void
                {
                    $title = $other->getTitle();
                    echo $title;
                }

                public function getTitle(): string
                {
                    return '';
                }
            }
        "#]],
    );
}

#[test]
fn a_name_the_scope_already_binds_is_not_shadowed() {
    // Shadowing changes what every use of that name below means, silently. The one thing a refactor
    // may not do is change the program.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(Report $other): void
    {
        $title = 'held';
        echo @from@$other->getTitle()@to@;
        echo $title;
    }

    public function getTitle(): string
    {
        return '';
    }
}
"#,
        expect![[r#"
            Extract $title2
            ----
            <?php
            namespace App;

            class Report
            {
                public function run(Report $other): void
                {
                    $title = 'held';
                    $title2 = $other->getTitle();
                    echo $title2;
                    echo $title;
                }

                public function getTitle(): string
                {
                    return '';
                }
            }
        "#]],
    );
}

#[test]
fn the_right_of_a_short_circuit_is_refused() {
    // `$flag && f()` runs `f()` only when `$flag` is truthy, and lifting it out runs it always.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(Report $other, bool $flag): void
    {
        $ok = $flag && @from@$other->getTitle()@to@;
    }

    public function getTitle(): string
    {
        return '';
    }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn an_arm_of_a_branch_is_refused() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(Report $other, bool $flag): void
    {
        $held = $flag ? @from@$other->getTitle()@to@ : '';
    }

    public function getTitle(): string
    {
        return '';
    }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_closures_body_is_extracted_into_the_closure_and_not_out_of_it() {
    // The closure's own statements are the frame: hoisting to the statement *outside* would move
    // the code out of the function it was written in, and hoisting inside is what was meant.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(Report $other): void
    {
        $make = function () use ($other) {
            return @from@$other->getTitle()@to@;
        };
    }

    public function getTitle(): string
    {
        return '';
    }
}
"#,
        expect![[r#"
            Extract $title
            ----
            <?php
            namespace App;

            class Report
            {
                public function run(Report $other): void
                {
                    $make = function () use ($other) {
                        $title = $other->getTitle();
                        return $title;
                    };
                }

                public function getTitle(): string
                {
                    return '';
                }
            }
        "#]],
    );
}

/// What is offered at each of several bare cursors, by title alone.
///
/// A second helper rather than a parameter on the one above, because what these ask is whether the
/// action appears at all: the edits of an offer that should not exist are not worth rendering, and
/// a refusal has none.
fn check_each(fixture: &str, markers: &[&str], expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);

    let mut out = String::new();
    for marker in markers {
        let position = spec.markers().position(marker);
        let file = world.file_id(&position.path);
        let rendered = with_open_scopes(&world, Some(file), |open| {
            let view = IndexView::in_file(world.skeleton(), open, file);
            let request = crate::ActionRequest {
                view: &view,
                at: TextRange::empty(position.offset),
                only: &[],
                asked: Default::default(),
            };
            code_actions(&request)
                .into_iter()
                // `Extract $name`, which is this file's subject -- and not `Extract 2 into a
                // constant`, which is a different offer that happens to share the first word.
                .find(|action| action.title.starts_with("Extract $"))
                .map_or_else(|| "(nothing offered)".to_owned(), |action| action.title)
        });
        let _ = writeln!(out, "@{marker} -> {rendered}");
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

/// Four places a selection sits inside a *declaration*, where a new statement cannot go.
///
/// A property default, a class constant, a parameter default and an attribute's argument are all
/// constant expressions. They are lowered as nested expressions of the unmodelled statement the
/// declaration becomes -- so goto and hover reach them -- and a reader that mistook that
/// containment for membership offered `$value = ['a' => 1]; class Other { public array $shape =
/// $value; }`, which PHP refuses.
///
/// Found by `tools/code-actions-apply-cleanly.py`, which applies every offer and lints the result.
#[test]
fn nothing_inside_a_declaration_is_offered() {
    check_each(
        r#"
//- /src/app.php
<?php
namespace App;

#[Marker(what: ['a' => 1, 'b' => @attribute@2])]
class Holder
{
    public array $shape = ['a' => 1, 'b' => @property@2];

    public const array KEYS = ['a' => 1, 'b' => @constant@2];

    public function takes(array $bag = ['a' => 1, 'b' => @parameter@2]): int
    {
        return count($bag);
    }
}

const TOP = ['a' => 1, 'b' => @top@2];
"#,
        &["attribute", "property", "constant", "parameter", "top"],
        expect![[r#"
            @attribute -> (nothing offered)
            @property -> (nothing offered)
            @constant -> (nothing offered)
            @parameter -> (nothing offered)
            @top -> (nothing offered)
        "#]],
    );
}

/// And the shapes inside a body that look like the ones above and are not: an array literal in an
/// argument is ordinary code, and lifting it is the whole point of the action.
#[test]
fn the_same_literal_inside_a_body_is_offered() {
    check_each(
        r#"
//- /src/app.php
<?php
namespace App;

class Holder
{
    public function runs(): int
    {
        return count(['a' => 1, 'b' => @here@2]);
    }
}
"#,
        &["here"],
        expect![[r#"
            @here -> Extract $value
        "#]],
    );
}

/// Three more shapes that are syntax rather than a value, each found by applying what was offered.
///
/// An arrow function's body is an *expression*, so there is nowhere in it to put a statement and
/// writing one there produced `fn ($k) => $value = f($k); $value)`. A write target is a place and
/// not a value: `$rows[] = $item` says where the item goes, and `$value = $rows[];` is not
/// something PHP can read.
///
/// The third is not a refusal and is here for the same reason: `...$args` is the *call's* syntax
/// and `$value = ...$args;` is not something PHP can read either, so the spread is no longer a
/// candidate -- and a cursor on it now reaches the call around it, which is a fine one.
#[test]
fn syntax_that_is_not_a_value_is_not_offered() {
    check_each(
        r#"
//- /src/app.php
<?php
namespace App;

class Holder
{
    public array $rows = [];

    public function appends(array $items): void
    {
        foreach ($items as $item) {
            $this->r@target@ows[] = $item['build'];
        }
    }

    public function arrows(array $keys): bool
    {
        return \array_any($keys, fn ($key) => !empty($this->r@arrow@ows[$key]));
    }

    public function spreads(array $args): void
    {
        $this->takes(...@spread@$args);
    }

    public function takes(int ...$numbers): void
    {
    }
}
"#,
        &["target", "arrow", "spread"],
        expect![[r#"
            @target -> (nothing offered)
            @arrow -> (nothing offered)
            @spread -> Extract $takes
        "#]],
    );
}

#[test]
fn an_elseif_condition_is_refused() {
    // `elseif` is not a statement boundary. Lowering turns each clause into an `if` of its own whose
    // range begins at the keyword, so the assignment lands between the previous arm's `}` and the
    // `elseif` -- and `code-actions-apply-cleanly` caught it on a real project as
    // `syntax error, unexpected token "elseif"`. Above the whole chain is the only other place it
    // could go, and there it would run on the path the chain exists to keep it off.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(Report $other, bool $flag): void
    {
        if ($flag) {
            $ok = 1;
        }
        elseif (@from@$other->getTitle()@to@) {
            $ok = 2;
        }
    }

    public function getTitle(): string
    {
        return '';
    }
}
"#,
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_nested_if_inside_an_else_block_is_still_offered() {
    // The control, and what tells the two apart: an `if` written inside `else { ... }` has braces
    // around it, so the statement above it is a place a statement can go. A synthesized `elseif`
    // owns everything after it including the final `else`, so its range ends where the whole chain
    // does -- which is the difference the refusal reads.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(Report $other, bool $flag): void
    {
        if ($flag) {
            $ok = 1;
        }
        else {
            if (@from@$other->getTitle()@to@) {
                $ok = 2;
            }
        }
    }

    public function getTitle(): string
    {
        return '';
    }
}
"#,
        expect![[r#"
            Extract $title
            ----
            <?php
            namespace App;

            class Report
            {
                public function run(Report $other, bool $flag): void
                {
                    if ($flag) {
                        $ok = 1;
                    }
                    else {
                        $title = $other->getTitle();
                        if ($title) {
                            $ok = 2;
                        }
                    }
                }

                public function getTitle(): string
                {
                    return '';
                }
            }
        "#]],
    );
}
