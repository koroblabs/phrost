//! The type a returned *variable* names, when this body is the only thing that gave it one.
//!
//! `returned_type` stops at a variable, because a variable names a type only after resolution --
//! and for the one shape that matters most that is not true. A factory with more than one line is
//! written
//!
//! ```php
//! private function createContainer()
//! {
//!     $container = new ContainerBuilder(new ParameterBag([...]));
//!     $container->registerExtension(new DebugExtension());
//!
//!     return $container;
//! }
//! ```
//!
//! and everything about what comes back is in the assignment three lines up, in the same body, in
//! the same file. Symfony writes 492 methods of this shape, Laravel 136 and WordPress 29 -- and each
//! one leaves every receiver downstream of the call untyped, which is how one test helper costs a
//! hundred unresolved accesses.
//!
//! # Every binding of the name, or nothing
//!
//! What makes this safe is that a *local* has a closed set of writers inside its own body, and this
//! refuses every one it cannot read rather than reasoning about the ones it can. A parameter is
//! written by the caller; a `foreach` target, a `catch` variable, a `global`, a `static` and a
//! `list()` destructuring each bind the name where no `new` is visible; a compound assignment
//! (`.=`, `??=`) combines with whatever was there. Any of those and the whole reading is given up.
//!
//! The one thing it cannot see is a by-reference argument -- `$this->fill($container)` may rebind
//! the name if the callee declared `&$container`, and which callee that is takes an index.
//! `slot_value` records the same unsoundness for the same reason and in the same words: what is
//! claimed is that nothing *in this body* wrote the name again.

use mago_syntax::cst::{
    Block, Expression, FunctionLikeParameterList, Node, UnaryPrefixOperator, Variable,
};
use phrost_hir::TypeRefId;

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    /// The type one returned expression names, reading a variable through this body's assignments.
    ///
    /// [`Lowerer::returned_type`] first, so the shapes that name a type outright keep the answer
    /// they always had; the variable reading is what used to be the end of the line.
    pub(crate) fn returned_type_id(
        &mut self,
        block: &Block<'p>,
        parameters: &FunctionLikeParameterList<'p>,
        value: &'p Expression<'p>,
    ) -> Option<TypeRefId> {
        if let Some(data) = self.returned_type(value) {
            return Some(self.builder.add_type_ref(data));
        }
        self.returned_local_type(block, parameters, value)
    }

    /// The one value a returned local was given, when this body assigns it exactly once and binds
    /// it no other way -- the same closed set of writers [`written_values`] reads for a type.
    pub(crate) fn the_one_value_of(
        &self,
        block: &Block<'p>,
        parameters: &FunctionLikeParameterList<'p>,
        value: &Expression<'p>,
    ) -> Option<&'p Expression<'p>> {
        let Expression::Variable(Variable::Direct(variable)) = value else {
            return None;
        };
        let name = variable.name;
        if parameters
            .parameters
            .iter()
            .any(|parameter| parameter.variable.name == name)
        {
            return None;
        }
        match written_values(block, name)?.as_slice() {
            [one] => Some(*one),
            _ => None,
        }
    }

    /// The type a returned variable holds, when this body's assignments agree on one.
    fn returned_local_type(
        &mut self,
        block: &Block<'p>,
        parameters: &FunctionLikeParameterList<'p>,
        value: &Expression<'p>,
    ) -> Option<TypeRefId> {
        let Expression::Variable(Variable::Direct(variable)) = value else {
            return None;
        };
        let name = variable.name;
        // A parameter arrives holding whatever the caller passed, and an assignment inside the body
        // is then one of two values rather than the only one.
        if parameters
            .parameters
            .iter()
            .any(|parameter| parameter.variable.name == name)
        {
            return None;
        }

        // Two assignments are two claims, and they are read as one type only when they name the
        // same one -- what `same_element` does for an array literal, for the same reason.
        let mut agreed: Option<TypeRefId> = None;
        for written in written_values(block, name)? {
            let data = self.returned_type(written)?;
            let id = self.builder.add_type_ref(data);
            match agreed {
                Some(seen) if !self.same_written_type(seen, id) => return None,
                _ => agreed = Some(id),
            }
        }
        agreed
    }
}

/// Every value plainly assigned to `name` in this body, or nothing when something else binds it.
///
/// The generic node walk, stepping around anything that opens a scope of its own, for the reason
/// `yields` states: a `$container` inside a closure is the closure's name and not this body's.
///
/// Empty is `None` too: a name with no assignment at all in the body is one bound somewhere this
/// walk did not look, and answering for it would be answering about a value nothing here wrote.
fn written_values<'ast, 'p>(
    block: &'ast Block<'p>,
    name: &'p [u8],
) -> Option<Vec<&'p Expression<'p>>> {
    let mut found: Vec<&'p Expression<'p>> = Vec::new();
    let mut queue = vec![Node::Block(block)];

    while let Some(node) = queue.pop() {
        match node {
            Node::Assignment(assignment) => {
                if names_directly(assignment.lhs, name) {
                    // A compound assignment combines with whatever was there, which is a value this
                    // walk never saw.
                    if !assignment.operator.is_assign() {
                        return None;
                    }
                    // `$a = &$b` binds the name to another variable's slot, so what it holds is
                    // whatever that one holds from here on.
                    if assignment.rhs.is_reference() {
                        return None;
                    }
                    found.push(assignment.rhs);
                } else if binds_within(assignment.lhs, name) {
                    // `[$a, $container] = $pair;` -- a destructuring target, which binds the name
                    // out of a value this does not take apart.
                    return None;
                }
                queue.extend(Node::Assignment(assignment).children());
            }
            // Every other way a body can bind a name. Each is a write this cannot read, and the
            // reading is given up rather than made from the assignments it *can* see.
            Node::ForeachValueTarget(target) if binds_within(target.value, name) => return None,
            Node::ForeachKeyValueTarget(target)
                if binds_within(target.key, name) || binds_within(target.value, name) =>
            {
                return None;
            }
            Node::TryCatchClause(clause) => {
                if clause
                    .variable
                    .as_ref()
                    .is_some_and(|variable| variable.name == name)
                {
                    return None;
                }
                queue.extend(Node::TryCatchClause(clause).children());
            }
            Node::Global(global) => {
                if global.variables.iter().any(|variable| match variable {
                    Variable::Direct(direct) => direct.name == name,
                    _ => false,
                }) {
                    return None;
                }
            }
            Node::StaticConcreteItem(item) if item.variable.name == name => return None,
            Node::StaticAbstractItem(item) if item.variable.name == name => return None,
            // A `++`/`--` rewrites the name with an arithmetic result, which is not a `new`.
            Node::UnaryPrefix(unary)
                if names_directly(unary.operand, name)
                    && matches!(
                        unary.operator,
                        UnaryPrefixOperator::PreIncrement(_)
                            | UnaryPrefixOperator::PreDecrement(_)
                            | UnaryPrefixOperator::Reference(_)
                    ) =>
            {
                return None;
            }
            Node::UnaryPostfix(postfix) if names_directly(postfix.operand, name) => return None,
            Node::Closure(_)
            | Node::ArrowFunction(_)
            | Node::Function(_)
            | Node::Method(_)
            | Node::PropertyHook(_)
            | Node::AnonymousClass(_)
            | Node::Class(_)
            | Node::Interface(_)
            | Node::Trait(_)
            | Node::Enum(_) => {}
            other => queue.extend(other.children()),
        }
    }

    (!found.is_empty()).then_some(found)
}

/// Whether an expression *is* the variable, rather than something reached through it.
///
/// `$container` is; `$container['k']` and `$container->field` are not -- writing through one of
/// those leaves the name holding the same object.
fn names_directly(expression: &Expression<'_>, name: &[u8]) -> bool {
    matches!(
        expression,
        Expression::Variable(Variable::Direct(variable)) if variable.name == name
    )
}

/// Whether an assignment target binds the name somewhere inside itself.
///
/// For the shapes that bind several names at once: `[$a, $b] = ...`, `list($a, $b) = ...`,
/// `foreach ($rows as [$id, $row])`. Anything holding the name is enough, because what is being
/// asked is whether this construct could have written it.
fn binds_within(target: &Expression<'_>, name: &[u8]) -> bool {
    if names_directly(target, name) {
        return true;
    }
    let mut queue = vec![Node::Expression(target)];
    while let Some(node) = queue.pop() {
        if let Node::DirectVariable(variable) = node
            && variable.name == name
        {
            return true;
        }
        queue.extend(node.children());
    }
    false
}
