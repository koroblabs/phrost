//! Folds that live in trivia: comments, and the region markers written inside them.
//!
//! Not in the tree at all, which is the reason this crate exists. A comment is attached to nothing,
//! so no layer above the parser can see one, and `#region` is a convention with no syntax behind it
//! whatsoever.

use mago_span::Span;
use mago_syntax::cst::Trivia;

use super::{FoldKind, FoldRange};

/// Every fold the file's comments contribute.
pub(super) fn collect<'a>(
    trivia: impl Iterator<Item = Trivia<'a>>,
    source: &str,
) -> Vec<FoldRange> {
    let comments = trivia
        .filter(|trivia| trivia.kind.is_comment())
        .collect::<Vec<_>>();

    let mut folds = regions(&comments, source);
    folds.extend(blocks(&comments));
    folds.extend(runs(&comments, source));
    folds
}

/// One fold per `/* */` or `/** */` comment. Single-line ones are dropped by the caller's filter.
fn blocks(comments: &[Trivia<'_>]) -> Vec<FoldRange> {
    comments
        .iter()
        .filter(|comment| comment.kind.is_block_comment())
        .map(|comment| FoldRange {
            range: phrost_syntax::text_range(comment.span),
            kind: FoldKind::Comment,
        })
        .collect()
}

/// One fold per run of `//` or `#` comments on consecutive lines.
///
/// Only comments that begin their own line join a run. A trailing `// why` after code is a remark
/// about that line, and folding from the middle of a statement to somewhere below it collapses code
/// the comment was never about.
///
/// A run of one is emitted like any other and dropped by the caller's single-line filter, so there
/// is no length rule here.
fn runs(comments: &[Trivia<'_>], source: &str) -> Vec<FoldRange> {
    let mut folds = Vec::new();
    let mut run: Option<(Span, Span)> = None;

    for comment in comments {
        let joins = comment.kind.is_single_line_comment()
            && marker(source, comment).is_none()
            && starts_its_line(source, comment.span);

        run = match (run, joins) {
            (Some((first, last)), true) if on_the_next_line(source, last, comment.span) => {
                Some((first, comment.span))
            }
            (Some((first, last)), _) => {
                folds.push(fold(first, last));
                joins.then_some((comment.span, comment.span))
            }
            (None, true) => Some((comment.span, comment.span)),
            (None, false) => None,
        };
    }
    folds.extend(run.map(|(first, last)| fold(first, last)));

    folds
}

/// One fold per matched `region` / `endregion` pair.
///
/// An unmatched `region` contributes nothing. Where it would end is a guess, and the two guesses
/// available — the end of the file, or the enclosing block — are both wrong often enough to be worse
/// than no fold arrow at all.
fn regions(comments: &[Trivia<'_>], source: &str) -> Vec<FoldRange> {
    let mut open: Vec<Span> = Vec::new();
    let mut folds = Vec::new();

    for comment in comments {
        match marker(source, comment) {
            Some(Marker::Start) => open.push(comment.span),
            Some(Marker::End) => {
                if let Some(start) = open.pop() {
                    folds.push(FoldRange {
                        range: phrost_syntax::text_range(start.join(comment.span)),
                        kind: FoldKind::Region,
                    });
                }
            }
            None => {}
        }
    }

    folds
}

/// Which end of a region a comment marks, if either.
enum Marker {
    Start,
    End,
}

/// Reads a comment as a region marker.
///
/// Three spellings are in use and all three arrive here as the same thing once the comment's own
/// leader is gone: `//region` from PhpStorm, `#region` from the editor-independent convention where
/// `#` happens to also be PHP's comment character, and `// #region` from people who learned it in a
/// language where `#` was not. `<editor-fold>` is PhpStorm's older form and costs one more test.
fn marker(source: &str, comment: &Trivia<'_>) -> Option<Marker> {
    if !comment.kind.is_single_line_comment() {
        return None;
    }

    let body = slice(source, comment.span)?.trim_start_matches(['/', '#', ' ', '\t']);

    if body.starts_with("endregion") || body.starts_with("</editor-fold") {
        Some(Marker::End)
    } else if body.starts_with("region") || body.starts_with("<editor-fold") {
        Some(Marker::Start)
    } else {
        None
    }
}

/// Whether only whitespace stands between the start of the line and this comment.
fn starts_its_line(source: &str, comment: Span) -> bool {
    let before = match source.get(..comment.start.offset as usize) {
        Some(before) => before,
        None => return false,
    };
    before
        .rsplit_once('\n')
        .map_or(before, |(_, last_line)| last_line)
        .trim()
        .is_empty()
}

/// Whether `next` begins on the line directly below the one `previous` ends on.
fn on_the_next_line(source: &str, previous: Span, next: Span) -> bool {
    source
        .get(previous.end.offset as usize..next.start.offset as usize)
        .is_some_and(|gap| gap.trim().is_empty() && gap.matches('\n').count() == 1)
}

fn fold(first: Span, last: Span) -> FoldRange {
    FoldRange {
        range: phrost_syntax::text_range(first.join(last)),
        kind: FoldKind::Comment,
    }
}

fn slice(source: &str, span: Span) -> Option<&str> {
    source.get(span.start.offset as usize..span.end.offset as usize)
}
