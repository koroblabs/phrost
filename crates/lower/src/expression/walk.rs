//! The generic unmodelled-node walk: what keeps an `Unmodelled` shape from swallowing its subtree.

use lspkit_arena::IdSlice;
use mago_span::HasSpan as _;
use mago_syntax::cst::{Attribute, Expression, Node};
use phrost_hir::{
    ExprId,
    body::{ClassTarget, ExprData},
};
use phrost_syntax::text_range;

use crate::lowerer::Lowerer;

/// One thing the generic walk found inside an unmodelled node.
///
/// Two shapes rather than one, because an attribute is written where an expression cannot be and is
/// still a call: [`Lowerer::nested_expressions`] sorts both by position and lowers each its own way.
enum Nested<'ast, 'arena> {
    Written(&'ast Expression<'arena>),
    Attribute(&'ast Attribute<'arena>),
}

impl Nested<'_, '_> {
    fn start(&self) -> phrost_model::TextSize {
        match self {
            Self::Written(expression) => text_range(expression.span()).start(),
            Self::Attribute(attribute) => text_range(attribute.span()).start(),
        }
    }
}

impl<'p> Lowerer<'_, 'p> {
    /// Every expression written inside a node, without modelling the node itself.
    ///
    /// What keeps an unmodelled shape from swallowing its subtree. `$this->attachmentTypes()[$id]` is
    /// an array access, which is not modelled -- and if that were the end of it, the method call
    /// inside would have no node at all, so goto on it would find nothing and look broken.
    ///
    /// Walks mago's generic node children until it reaches something that is an expression, and
    /// lowers that. The outermost expressions only: each one lowers its own insides.
    ///
    /// It steps around a function-like body. A class or function declaration is itself a statement,
    /// so descending into one would pull every expression in every method body into the *file's*
    /// body as well as into the method's own -- doubling the body IR of a typical PHP file, and
    /// putting those expressions in a scope they are not written in. What surrounds the body still
    /// belongs here: a parameter default is written in the declaration, not in the body, so
    /// `function f($n = self::LIMIT)` has nowhere else to be lowered.
    pub(crate) fn nested_expressions(&mut self, node: Node<'_, 'p>) -> IdSlice<ExprId> {
        let mut expressions = Vec::new();
        let mut queue = node.children();

        while let Some(child) = queue.pop() {
            match child {
                Node::Expression(expression) => expressions.push(Nested::Written(expression)),
                // An attribute is a constructor call written where no expression walk would find
                // one, so it is taken whole rather than descended into: its arguments belong to the
                // call, and reaching them separately would lower each of them twice.
                Node::Attribute(attribute) => expressions.push(Nested::Attribute(attribute)),
                Node::Method(method) => {
                    queue.extend(Node::Method(method).children().into_iter().filter(|child| {
                        !matches!(child, Node::MethodBody(_) | Node::MethodAbstractBody(_))
                    }))
                }
                // A property hook is function-like too: `set(Item $value) => $this->x = $value` has
                // its own parameter, and pulling its body into the file's would put a `$value`
                // nothing binds where a check about unbound names can see it. The hook's own
                // parameters stay -- a default is written in the declaration and has nowhere else to
                // go, the same reason a method's do.
                Node::PropertyHook(hook) => queue.extend(
                    Node::PropertyHook(hook)
                        .children()
                        .into_iter()
                        .filter(|child| {
                            !matches!(
                                child,
                                Node::PropertyHookBody(_)
                                    | Node::PropertyHookAbstractBody(_)
                                    | Node::PropertyHookConcreteBody(_)
                                    | Node::PropertyHookConcreteExpressionBody(_)
                            )
                        }),
                ),
                // A free function's body is a plain block, which is not distinguishable by node
                // kind -- so it is identified by being the one at the body's span.
                Node::Function(function) => {
                    let body = function.body.span();
                    queue.extend(
                        Node::Function(function)
                            .children()
                            .into_iter()
                            .filter(|child| child.span() != body),
                    );
                }
                other => queue.extend(other.children()),
            }
        }

        // Source order, because the queue is a stack and because a snapshot has to be stable.
        expressions.sort_by_key(|nested| nested.start());
        let ids = expressions
            .into_iter()
            .filter_map(|nested| match nested {
                Nested::Written(expression) => Some(self.expression(expression)),
                Nested::Attribute(attribute) => self.attribute_call(attribute),
            })
            .collect::<Vec<_>>();
        self.builder.alloc_expr_list(&ids)
    }

    /// An attribute, lowered as the constructor call it is.
    ///
    /// `#[Route("/x", name: "home")]` runs `new Route("/x", name: "home")`, so everything that reads
    /// a call -- signature help, named-argument completion, the parameter hints -- has the same work
    /// to do and no reason to know an attribute is different. The class comes from the reference the
    /// declaration pass already recorded; see [`Lowerer::attribute_ref_at`].
    ///
    /// `None` when no reference was recorded, which is a position that never asked for bodies.
    fn attribute_call(&mut self, attribute: &Attribute<'p>) -> Option<ExprId> {
        let class = self.attribute_ref_at(self.within_the_text(attribute.name.span()).start())?;
        let args = self.partial_arguments(attribute.argument_list.as_ref());
        Some(self.builder.add_expr(ExprData::New {
            class: ClassTarget::Named(class),
            args,
            range: self.within_the_text(attribute.span()),
        }))
    }
}
