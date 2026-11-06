//! The run of `use` statements at the top of a file, as one collapsible block.
//!
//! Its own kind in the protocol, because an editor may collapse imports on open and nothing else.

use mago_span::{HasSpan, Span};
use mago_syntax::cst::Node;
use phrost_model::TextRange;

/// The span of one `use` statement, terminator included.
pub(super) fn use_statement(node: Node<'_, '_>) -> Option<Span> {
    match node {
        Node::Use(statement) => Some(statement.span()),
        _ => None,
    }
}

/// Groups the collected statements into runs and returns one range per run.
///
/// A run is broken by anything written between two `use` lines, a comment included. That is
/// stricter than it has to be, and deliberately: a comment between imports is nearly always a
/// heading for the group below it, so treating it as a break gives two blocks that each mean
/// something instead of one that hides the heading.
pub(super) fn runs(statements: &mut [Span], source: &str) -> Vec<TextRange> {
    statements.sort_by_key(|span| span.start.offset);

    let mut runs = Vec::new();
    let mut current: Option<(u32, u32)> = None;

    for span in statements.iter() {
        current = match current {
            Some((start, end)) if only_whitespace(source, end, span.start.offset) => {
                Some((start, span.end.offset))
            }
            Some((start, end)) => {
                runs.push((start, end));
                Some((span.start.offset, span.end.offset))
            }
            None => Some((span.start.offset, span.end.offset)),
        };
    }
    runs.extend(current);

    runs.into_iter()
        .map(|(start, end)| TextRange::new(start.into(), end.into()))
        .collect()
}

/// Whether the gap between two statements holds nothing but whitespace.
fn only_whitespace(source: &str, from: u32, to: u32) -> bool {
    source
        .get(from as usize..to as usize)
        .is_some_and(|gap| gap.trim().is_empty())
}
