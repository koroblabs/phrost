//! Which ranges of a file an editor may collapse.
//!
//! Three sources, none of which can answer for the others: the tree, for everything that opens and
//! closes; the `use` statements, grouped into the one block an editor collapses on open; and the
//! trivia, for comments and the `#region` markers written inside them.
//!
//! Nothing here needs the project, so nothing here waits for it. Folding answers from the buffer
//! alone, which means it works in the second before indexing finishes and in a file that is briefly
//! not valid PHP — the two moments an editor asks most.

use mago_syntax::cst::Node;
use phrost_model::TextRange;
use phrost_syntax::ParseSession;

mod braces;
mod comments;
mod imports;

#[cfg(test)]
mod tests;

/// One collapsible range, and why it is one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FoldRange {
    /// From the opening delimiter to the closing one, both included.
    ///
    /// Byte offsets rather than lines. Turning one into a line and a column belongs to the protocol
    /// layer, which is the only layer that knows which position encoding the client agreed to.
    pub range: TextRange,
    pub kind: FoldKind,
}

/// What kind of thing folds, in the protocol's vocabulary.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FoldKind {
    /// A delimiter pair: a body, a declaration, an array, an argument list. The protocol has no
    /// name for this case and expects no kind to be sent for it.
    Block,
    Imports,
    Comment,
    Region,
}

/// Every range in `text` an editor may collapse, in document order.
///
/// Ranges nest and overlap freely — a method inside a class inside a namespace is three of them —
/// which is what the protocol expects. What is filtered out is the range that fits on one line:
/// there is nothing to collapse, and an editor would still draw an arrow next to it.
pub fn ranges(text: &str) -> Vec<FoldRange> {
    let session = ParseSession::new();
    let parsed = session.parse(text);
    let source = parsed.source();

    let mut folds = Vec::new();
    let mut use_statements = Vec::new();

    // An explicit worklist rather than recursion. Nesting depth here is attacker-controlled in the
    // sense that matters for an editor -- a generated file can hold a thousand-deep expression --
    // and a stack overflow takes the whole server with it, not just the request.
    let mut pending = vec![Node::Program(parsed.program())];
    while let Some(node) = pending.pop() {
        if let Some(range) = braces::fold_of(node) {
            folds.push(FoldRange {
                range,
                kind: FoldKind::Block,
            });
        }
        if let Some(span) = imports::use_statement(node) {
            use_statements.push(span);
        }
        node.visit_children(|child| pending.push(child));
    }

    folds.extend(
        imports::runs(&mut use_statements, source)
            .into_iter()
            .map(|range| FoldRange {
                range,
                kind: FoldKind::Imports,
            }),
    );
    folds.extend(comments::collect(
        parsed.program().trivia.iter().copied(),
        source,
    ));

    folds.retain(|fold| spans_more_than_one_line(source, fold.range));
    folds.sort_by_key(|fold| (fold.range.start(), fold.range.end()));
    folds.dedup();
    folds
}

/// Whether a range crosses a line boundary, which is what makes it worth collapsing.
fn spans_more_than_one_line(source: &str, range: TextRange) -> bool {
    source
        .get(usize::from(range.start())..usize::from(range.end()))
        .is_some_and(|text| text.contains('\n'))
}
