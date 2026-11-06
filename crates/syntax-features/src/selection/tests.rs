//! The chain a selection grows through, rendered as the text of each step.
//!
//! Rendered rather than asserted as offsets: what matters about a step is what a person would see
//! highlighted, and a pair of numbers does not say.

use expect_test::{Expect, expect};
use phrost_model::TextSize;

use super::ranges_at;

/// Every step at the `$0` marker, from the innermost outward, each shown as its text.
fn check(source: &str, expect: Expect) {
    let offset = source.find("$0").expect("the fixture marks a position");
    let text = source.replace("$0", "");
    let rendered = ranges_at(&text, TextSize::from(u32::try_from(offset).unwrap()))
        .into_iter()
        .map(|range| {
            let step = &text[usize::from(range.start())..usize::from(range.end())];
            // One line, because a step that spans a method is otherwise most of the snapshot.
            // Trimmed at the end and marked with `...`, so no rendered line carries trailing
            // whitespace -- a snapshot that does cannot round-trip through `UPDATE_EXPECT`, which
            // strips it, and the test then fails the moment it is accepted.
            // Bulleted, like every other rendering in this project -- and not indented by hand:
            // `expect!` dedents by the smallest indent it finds, so a leading pair of spaces on
            // every line is stripped on write and the test fails the moment it is accepted.
            let first = step.split('\n').next().unwrap_or("").trim_end();
            let shown = match (first.is_empty(), step.contains('\n')) {
                (true, _) => "...".to_owned(),
                (false, true) => format!("{first} ..."),
                (false, false) => first.to_owned(),
            };
            format!("- {shown}\n")
        })
        .collect::<String>();
    expect.assert_eq(&rendered);
}

#[test]
fn a_selection_grows_from_the_name_to_the_declaration() {
    check(
        r#"<?php

namespace App;

class Repo
{
    public function run(): void
    {
        $this->fi$0nd(1);
    }
}
"#,
        expect![[r#"
            - find
            - $this->find(1)
            - $this->find(1);
            - { ...
            - public function run(): void ...
            - class Repo ...
            - ; ...
            - namespace App; ...
            - <?php ...
        "#]],
    );
}

#[test]
fn text_outside_php_still_belongs_to_the_file() {
    // Inline HTML after a closing tag is a node of the program, so there is a chain -- just a short
    // one. Answering nothing here would be wrong: the text is in the file and selecting it is a
    // thing a person does.
    check(
        "<?php\n$a = 1;\n?>\nplain text $0here\n",
        expect![[r#"
        - ...
        - <?php ...
    "#]],
    );
}

#[test]
fn each_step_is_strictly_larger_than_the_one_before() {
    // The property the feature rests on: a keystroke that selects what was already selected reads
    // as the keystroke doing nothing, so a duplicate range is a bug rather than a harmless extra.
    let text = "<?php\n\nnamespace App;\n\nclass Repo\n{\n    public function run(): array\n    {\n        return ['a' => $this->id(), 'b' => 2];\n    }\n}\n";
    for offset in 0..text.len() {
        let steps = ranges_at(text, TextSize::from(u32::try_from(offset).unwrap()));
        for pair in steps.windows(2) {
            assert!(
                pair[0].len() < pair[1].len(),
                "at {offset}: {:?} does not grow into {:?}",
                pair[0],
                pair[1]
            );
        }
    }
}
