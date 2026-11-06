//! The nested ranges an editor grows a selection through.
//!
//! One keystroke should take a selection from the name under the cursor to the expression around it,
//! then the statement, then the block, then the declaration. That is the syntax tree read as a chain
//! rather than as a tree, and it needs nothing else: no project, no types, no index. So it answers
//! from the buffer while indexing is still running, which is the same reason folding does.
//!
//! Unlike folding there is no subset to choose. Every node the tree has is a step somebody might
//! want, and a shape left out of a list here is a step that silently jumps two levels.

use mago_span::HasSpan as _;
use mago_syntax::cst::Node;
use phrost_model::{TextRange, TextSize};
use phrost_syntax::{ParseSession, text_range};

#[cfg(test)]
mod tests;

/// Every range containing `offset`, innermost first, each strictly inside the next.
///
/// Empty for a position no node covers, which is a position outside the PHP the parser found -- past
/// a closing tag, or in a file that is not PHP at all.
pub fn ranges_at(text: &str, offset: TextSize) -> Vec<TextRange> {
    let session = ParseSession::new();
    let parsed = session.parse(text);

    let mut found = Vec::new();
    // A worklist rather than recursion, for the reason folding gives: nesting depth in a generated
    // file is unbounded and a stack overflow takes the server rather than the request.
    let mut pending = vec![Node::Program(parsed.program())];
    while let Some(node) = pending.pop() {
        let range = text_range(node.span());
        if !range.contains_inclusive(offset) {
            // Children are inside their parent, so a node that misses cannot hold one that hits.
            // This is what keeps the walk to the path from the root rather than the whole file.
            continue;
        }
        found.push(range);
        node.visit_children(|child| pending.push(child));
    }

    // Smallest first, then built into a chain rather than left as a list.
    //
    // Two reasons it cannot just be sorted. The tree has nodes that wrap a single child with no
    // extra text -- a statement holding one expression -- and a step that selects exactly what was
    // already selected reads as the keystroke doing nothing. And at a boundary two nodes touch the
    // cursor from opposite sides: in `$a->b` with the cursor between `$a` and `->`, one range ends
    // there and another begins, both the same size and neither inside the other. The protocol wants
    // a parent chain, so a step is kept only when it contains the step before it -- which settles
    // the boundary by keeping whichever comes first in this order, deterministically.
    found.sort_by_key(|range| (range.len(), range.start()));
    found.dedup();

    let mut chain: Vec<TextRange> = Vec::new();
    for range in found {
        let grows = match chain.last() {
            None => true,
            Some(inner) => range.contains_range(*inner) && range.len() > inner.len(),
        };
        if grows {
            chain.push(range);
        }
    }
    chain
}
