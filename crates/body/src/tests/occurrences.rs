//! Every place one local name is written, and the two places it stops.
//!
//! The completeness half is what a rename depends on: a `catch` clause and a parameter bind a name
//! without writing an expression, so nothing that walks expressions alone would find them -- and a
//! rename that missed one would leave the file unparseable.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner, TextSize};

use crate::{OccurrenceKind, local_occurrences};

/// Renders every occurrence of `variable` at the `$0` marker, as `line:col-col read|write`.
fn check(source: &str, variable: &str, expect: Expect) {
    let offset = TextSize::try_from(source.find("$0").expect("the source marks a position"))
        .expect("fixture fits in u32");
    let text = source.replace("$0", "");

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
    let rendered = local_occurrences(&lowered.hir, offset, variable)
        .into_iter()
        .map(|occurrence| {
            let start = lines.line_col(occurrence.range.start());
            let end = lines.line_col(occurrence.range.end());
            format!(
                "{}:{}-{} {}\n",
                start.line + 1,
                start.col + 1,
                end.col + 1,
                match occurrence.kind {
                    OccurrenceKind::Read => "read",
                    OccurrenceKind::Write => "write",
                    OccurrenceKind::ReadWrite => "read+write",
                }
            )
        })
        .collect::<String>();

    expect.assert_eq(&rendered);
}

#[test]
fn a_parameter_and_every_read_of_it() {
    check(
        r#"<?php
class Repo
{
    public function run(int $count): int
    {
        $co$0unt = $count + 1;
        return $count;
    }
}
"#,
        "count",
        expect![[r#"
            4:29-35 write
            6:9-15 write
            6:18-24 read
            7:16-22 read
        "#]],
    );
}

#[test]
fn a_catch_clause_binds_without_writing_an_expression() {
    // The name lives on the clause rather than in the body, so an expression walk alone sees the
    // reads and not the binding -- and `catch (Throwable $e)` left alone by a rename is a parse
    // error.
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        try {
            $this->work();
        }
        catch (\Throwable $e) {
            $this->log($e$0->getMessage());
        }
    }
}
"#,
        "e",
        expect![[r#"
            9:27-29 write
            10:24-26 read
        "#]],
    );
}

#[test]
fn a_name_compact_collects_is_read_inside_the_quotes() {
    // The string is the variable's name, so a rename has to change it: left behind, the call
    // stops collecting the variable and nothing says so. `extract()` is not here -- what it
    // writes depends on the array's keys at runtime, not on a name the call spells.
    check(
        r#"<?php
function render(array $rows): array
{
    $ti$0tle = 'Report';
    $count = count($rows);
    return \compact('title', 'count', "title", 'titles') + extract(['title' => 1]);
}
"#,
        "title",
        expect![[r#"
            4:5-11 write
            6:22-27 read
            6:40-45 read
        "#]],
    );
}

/// A closure that captures by reference is entered the way one capturing by value is.
///
/// `use (&$fail)` hands the closure the variable itself, so every `$fail` in its body is this one
/// -- and the walk deciding which bodies to enter matched the capture as a bare variable, which the
/// `&` wraps. So the capture was listed and the body was not: references from `$fail++` inside the
/// closure did not include it, and a rename changed the name outside and in the `use` and left the
/// body spelling the old one. Asked from the outer write and from inside, because both have to agree.
#[test]
fn a_capture_by_reference_reaches_into_the_closure() {
    let source = r#"<?php
function run(): int
{
    $fail = 0;
    $out = function (string $status) use (&$fail): void {
        if ($status === 'FAIL') {
            $fail++;
        }
    };
    $out('FAIL');
    return $fail;
}
"#;
    check(
        &source.replacen("$fail = 0", "$0$fail = 0", 1),
        "fail",
        expect![[r#"
            4:5-10 write
            5:44-49 read+write
            7:13-18 read+write
            11:12-17 read
        "#]],
    );
    check(
        &source.replacen("$fail++", "$0$fail++", 1),
        "fail",
        expect![[r#"
            4:5-10 write
            5:44-49 read+write
            7:13-18 read+write
            11:12-17 read
        "#]],
    );
}
