//! A parameter's own name answers the same as a use of it.
//!
//! The scope chain resolved to the function *body*, and a signature is not inside the body -- so
//! every question about the local was silence at the one position a person renames a parameter
//! from, and correct one line down. Rename made no edit and reported success; document highlight,
//! references and goto-type-definition were all dead at the same offset.
//!
//! So each case asks from both positions and renders a block per position. Two identical blocks is
//! the property. A filled block beside an empty one is the bug, and it is invisible to a test that
//! asks from only one of them.
//!
//! # A declaration with no body is a scope too
//!
//! The same silence, one layer out. An `interface`'s method and an `abstract` one bind parameters
//! and hold no statements, and the scan that finds the enclosing declaration was keyed on *having*
//! a body -- so the one word a person reads an API by had no highlight, no rename and no
//! goto-type-definition at all. Those cases have a single position to ask from, since there is
//! nowhere else the name can be written.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner, TextSize};

use crate::{OccurrenceKind, local_occurrences};

/// Renders the occurrences of `variable` as asked from `$0` and then from `$1`.
fn check(source: &str, variable: &str, expect: Expect) {
    let mut text = source.to_owned();
    let mut offsets = Vec::new();
    for marker in ["$0", "$1"] {
        let at = text
            .find(marker)
            .unwrap_or_else(|| panic!("the source marks {marker}"));
        text = text.replacen(marker, "", 1);
        offsets.push(TextSize::try_from(at).expect("fixture fits in u32"));
    }

    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        &text,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );

    let lines = phrost_model::LineIndex::new(&text);
    let mut out = String::new();
    for (marker, offset) in ["$0", "$1"].iter().zip(offsets) {
        out.push_str(&format!("from {marker}\n"));
        for occurrence in local_occurrences(&lowered.hir, offset, variable) {
            let start = lines.line_col(occurrence.range.start());
            let end = lines.line_col(occurrence.range.end());
            out.push_str(&format!(
                "  {}:{}-{} {}\n",
                start.line + 1,
                start.col + 1,
                end.col + 1,
                match occurrence.kind {
                    OccurrenceKind::Read => "read",
                    OccurrenceKind::Write => "write",
                    OccurrenceKind::ReadWrite => "read+write",
                }
            ));
        }
    }

    expect.assert_eq(&out);
}

/// The same, for a declaration whose parameter is written exactly once.
fn check_alone(source: &str, variable: &str, expect: Expect) {
    let at = source.find("$0").expect("the source marks $0");
    let text = source.replacen("$0", "", 1);
    let offset = TextSize::try_from(at).expect("fixture fits in u32");

    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        &text,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );

    let lines = phrost_model::LineIndex::new(&text);
    let mut out = String::new();
    for occurrence in local_occurrences(&lowered.hir, offset, variable) {
        let start = lines.line_col(occurrence.range.start());
        let end = lines.line_col(occurrence.range.end());
        // No leading padding: every line of this block is content, and expect-test cannot tell
        // indentation it has to strip from indentation that belongs to the answer. See
        // CLAUDE.md §8.4.
        out.push_str(&format!(
            "{}:{}-{} {}\n",
            start.line + 1,
            start.col + 1,
            end.col + 1,
            match occurrence.kind {
                OccurrenceKind::Read => "read",
                OccurrenceKind::Write => "write",
                OccurrenceKind::ReadWrite => "read+write",
            }
        ));
    }

    expect.assert_eq(&out);
}

#[test]
fn an_interface_methods_parameter_answers_from_its_own_name() {
    check_alone(
        r#"<?php
interface Formula
{
    public function score(int $we$0ight): float;
}
"#,
        "weight",
        expect![[r#"
            4:31-38 write
        "#]],
    );
}

#[test]
fn an_abstract_methods_parameter_answers_too() {
    check_alone(
        r#"<?php
abstract class Base
{
    abstract public function build(int $de$0pth): string;
}
"#,
        "depth",
        expect![[r#"
            4:40-46 write
        "#]],
    );
}

/// The refusal beside them: a bodyless declaration holds no statements, so a name written in the
/// *class* around it is a different variable and must not be swept in.
#[test]
fn a_bodyless_declaration_reaches_no_further_than_its_own_signature() {
    check_alone(
        r#"<?php
abstract class Base
{
    abstract public function build(int $de$0pth): string;

    public function run(int $depth): int
    {
        return $depth;
    }
}
"#,
        "depth",
        expect![[r#"
            4:40-46 write
        "#]],
    );
}

#[test]
fn a_method_parameter_answers_from_its_own_name() {
    check(
        r#"<?php
class Repo
{
    public function run(int $co$0unt): int
    {
        $co$1unt += 1;
        return $count;
    }
}
"#,
        "count",
        expect![[r#"
            from $0
              4:29-35 write
              6:9-15 read+write
              7:16-22 read
            from $1
              4:29-35 write
              6:9-15 read+write
              7:16-22 read
        "#]],
    );
}

#[test]
fn a_closure_parameter_answers_from_its_own_name_and_stays_inside_the_closure() {
    check(
        r#"<?php
class Repo
{
    public function run(array $rows): void
    {
        $row = 1;
        $mapped = array_map(function (int $r$0ow): int {
            return $r$1ow + 1;
        }, $rows);
    }
}
"#,
        "row",
        expect![[r#"
            from $0
              7:43-47 write
              8:20-24 read
            from $1
              7:43-47 write
              8:20-24 read
        "#]],
    );
}

#[test]
fn a_property_hooks_parameter_answers_from_its_own_name() {
    check(
        r#"<?php
class Offer
{
    public string $label = '' {
        set(string $va$0lue) {
            $this->label = trim($va$1lue);
        }
    }
}
"#,
        "value",
        expect![[r#"
            from $0
              5:20-26 write
              6:33-39 read
            from $1
              5:20-26 write
              6:33-39 read
        "#]],
    );
}
