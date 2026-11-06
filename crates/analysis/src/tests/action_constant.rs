//! `actions::constant`: a literal lifted into a class constant.
//!
//! The *file the edits produce* is snapshotted, because the constant and the reference to it are one
//! change: either alone leaves a class that names something it does not declare.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_model::TextRange;
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

fn check(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let markers = spec.markers();
    let from = markers.position("from");
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
            .find(|action| action.title.contains("into a constant"))
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
fn a_string_is_named_after_what_it_says_and_goes_above_the_properties() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    private string $held = 'x';

    public function run(): void
    {
        echo @from@'reduced payment'@to@;
    }
}
"#,
        expect![[r#"
            Extract 'reduced payment' into a constant
            ----
            <?php
            namespace App;

            class Report
            {
                private const REDUCED_PAYMENT = 'reduced payment';

                private string $held = 'x';

                public function run(): void
                {
                    echo self::REDUCED_PAYMENT;
                }
            }
        "#]],
    );
}

#[test]
fn a_number_has_no_words_to_take_and_gets_one_to_rename() {
    // Nothing in `4096` says what it is for, and a generated `CONST_4096` would only look as though
    // it did.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(): void
    {
        echo @from@4096@to@;
    }
}
"#,
        expect![[r#"
            Extract 4096 into a constant
            ----
            <?php
            namespace App;

            class Report
            {
                private const VALUE = 4096;

                public function run(): void
                {
                    echo self::VALUE;
                }
            }
        "#]],
    );
}

#[test]
fn a_name_the_class_already_declares_is_numbered_rather_than_repeated() {
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    private const REDUCED = 'other';

    public function run(): void
    {
        echo @from@'reduced'@to@;
    }
}
"#,
        expect![[r#"
            Extract 'reduced' into a constant
            ----
            <?php
            namespace App;

            class Report
            {
                private const REDUCED_2 = 'reduced';

                private const REDUCED = 'other';

                public function run(): void
                {
                    echo self::REDUCED_2;
                }
            }
        "#]],
    );
}

#[test]
fn a_keyword_is_already_as_plain_as_a_name_can_make_it() {
    // A constant standing for `true` says less than the word does.
    check(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function run(): void
    {
        echo @from@true@to@;
    }
}
"#,
        expect!["(nothing offered)\n"],
    );
}
