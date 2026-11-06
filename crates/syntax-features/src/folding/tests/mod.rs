//! What folds, rendered whole.
//!
//! One file per source of ranges, because the three sources fail in unrelated ways and a shared
//! expected-output file is the one thing every change to any of them would touch.

mod braces;
mod comments;
mod imports;
mod invariants;

use expect_test::Expect;

use crate::folding::{FoldKind, ranges};

/// Renders every fold of `source` as its kind, its span in lines and columns, and the line it
/// starts on.
///
/// The starting *line*, not the text the range begins with — a brace fold begins at `{`, and
/// `{` on its own says nothing about what folded.
pub(super) fn check(source: &str, expected: Expect) {
    let rendered = ranges(source)
        .into_iter()
        .map(|fold| {
            let (start_line, start_column) = position(source, usize::from(fold.range.start()));
            let (end_line, end_column) = position(source, usize::from(fold.range.end()));
            format!(
                "{:<7} {start_line}:{start_column}-{end_line}:{end_column}  {}\n",
                name(fold.kind),
                line(source, start_line),
            )
        })
        .collect::<String>();

    expected.assert_eq(&rendered);
}

fn name(kind: FoldKind) -> &'static str {
    match kind {
        FoldKind::Block => "block",
        FoldKind::Imports => "imports",
        FoldKind::Comment => "comment",
        FoldKind::Region => "region",
    }
}

/// One-based line, zero-based column in characters.
fn position(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset];
    let column = before
        .rsplit_once('\n')
        .map_or(before, |(_, last)| last)
        .chars()
        .count();
    (before.matches('\n').count() + 1, column)
}

fn line(source: &str, number: usize) -> &str {
    source.lines().nth(number - 1).unwrap_or("").trim()
}
