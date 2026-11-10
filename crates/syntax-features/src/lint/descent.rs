//! The node a finding is about, found again in the tree.
//!
//! A rule hands over spans and a sentence. A filter that has to judge the *expression* -- which
//! arguments a call was written with, what an `@` is in front of -- needs the node back, and the
//! span is the only name it has for it.

use mago_span::{HasSpan, Span};
use mago_syntax::cst::{Node, Program};

/// One path down the tree, to whatever holds a span.
pub(super) struct Descent;

impl Descent {
    /// The first thing `pick` makes of a node whose span holds `span`, from the root down.
    ///
    /// Only into nodes that hold the span, so this is a walk down one path of the tree rather than
    /// over all of it: a lint runs on every keystroke of an open buffer, and a generated file may
    /// hold one finding a thousand times. A worklist rather than recursion, for the same file.
    ///
    /// That leans on a parent's span covering its children's, which was checked rather than
    /// assumed: of 151 936 function calls in Symfony and WordPress, found by walking everything,
    /// this reached every one. Where it does not reach, the answer is nothing, and a filter with
    /// nothing leaves the finding as the rule wrote it.
    pub(super) fn to<'arena, T>(
        program: &'arena Program<'arena>,
        span: Span,
        pick: impl Fn(Node<'arena, 'arena>) -> Option<T>,
    ) -> Option<T> {
        let mut pending = vec![Node::Program(program)];
        while let Some(node) = pending.pop() {
            if let Some(picked) = pick(node) {
                return Some(picked);
            }
            node.visit_children(|child| {
                let held = child.span();
                if held.start.offset <= span.start.offset && span.end.offset <= held.end.offset {
                    pending.push(child);
                }
            });
        }
        None
    }
}
