//! The delimiter pairs an editor may collapse.
//!
//! One arm per shape that opens and closes, taking the two delimiter spans rather than the node's
//! own. The difference shows on a declaration whose header wraps: from the node's span a fold hides
//! the signature it is named by, and from the brace it does not.

use mago_span::HasSpan;
use mago_syntax::cst::Node;
use phrost_model::TextRange;

/// The range `node` contributes, or `None` when it is not a shape that opens and closes.
///
/// A default of "not foldable" rather than an exhaustive match, which is the one place in this
/// project that is the right way round: mago's `Node` is `#[non_exhaustive]` and holds every shape
/// in the language, so the list of shapes worth folding is a deliberate subset and a new mago node
/// should not be a compile error here.
pub(super) fn fold_of(node: Node<'_, '_>) -> Option<TextRange> {
    let span = match node {
        // Statement blocks. This one arm carries free function bodies, method bodies, every
        // control-flow body written with braces, `try`/`catch`/`finally`, and a braced `namespace`
        // — all of them are a `Block` in the tree, so none needs an arm of its own.
        Node::Block(block) => block.left_brace.join(block.right_brace),

        // Declaration bodies.
        Node::Class(class) => class.left_brace.join(class.right_brace),
        Node::Interface(interface) => interface.left_brace.join(interface.right_brace),
        Node::Trait(r#trait) => r#trait.left_brace.join(r#trait.right_brace),
        Node::Enum(r#enum) => r#enum.left_brace.join(r#enum.right_brace),
        Node::AnonymousClass(class) => class.left_brace.join(class.right_brace),
        Node::PropertyHookList(hooks) => hooks.left_brace.join(hooks.right_brace),
        Node::TraitUseConcreteSpecification(spec) => spec.left_brace.join(spec.right_brace),

        // Expressions and clauses with a brace body.
        Node::Match(r#match) => r#match.left_brace.join(r#match.right_brace),
        Node::SwitchBraceDelimitedBody(body) => body.left_brace.join(body.right_brace),

        // A group `use A\{B, C};` written over several lines.
        Node::MixedUseItemList(items) => items.left_brace.join(items.right_brace),
        Node::TypedUseItemList(items) => items.left_brace.join(items.right_brace),

        // Literals and lists. Arrays earn their place on a Drupal codebase, where a render array
        // is the largest thing in most files.
        Node::Array(array) => array.left_bracket.join(array.right_bracket),
        Node::LegacyArray(array) => array.left_parenthesis.join(array.right_parenthesis),
        Node::List(list) => list.left_parenthesis.join(list.right_parenthesis),
        Node::ArgumentList(arguments) => {
            arguments.left_parenthesis.join(arguments.right_parenthesis)
        }
        Node::FunctionLikeParameterList(parameters) => parameters
            .left_parenthesis
            .join(parameters.right_parenthesis),

        // The alternative syntax — `foreach (...): ... endforeach;` — and heredocs. These have no
        // delimiter pair to name: the whole node *is* the region, from the colon or the opening
        // marker to the closing keyword.
        Node::IfColonDelimitedBody(_)
        | Node::ForeachColonDelimitedBody(_)
        | Node::ForColonDelimitedBody(_)
        | Node::WhileColonDelimitedBody(_)
        | Node::SwitchColonDelimitedBody(_)
        | Node::DeclareColonDelimitedBody(_)
        | Node::DocumentString(_) => node.span(),

        _ => return None,
    };

    Some(phrost_syntax::text_range(span))
}
