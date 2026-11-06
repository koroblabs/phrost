//! Walking a lowered body.
//!
//! Every question about a body — which locals are bound, where a member is named, whether an offset
//! sits in a string — is a walk over the same tree, so the walks live here and the questions live
//! next to what they answer. One traversal to get right rather than one per question.
//!
//! Closure bodies are deliberately *not* descended into: a closure is its own scope, and a caller
//! that wants one asks for it. Unmodelled nodes *are* descended into, because what is written inside
//! a shape this IR does not model is still written.

use lspkit_arena::IdSlice;
#[cfg(test)]
use phrost_hir::body::LogicalOp;
use phrost_hir::{
    ExprId, FileHir, StmtId,
    body::{ClassTarget, ExprData, StmtData},
};
use phrost_model::{TextRange, TextSize};

/// Visits the key and value targets of every `foreach` written before `offset`.
pub(crate) fn visit_foreach_targets(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    offset: TextSize,
    visit: &mut impl FnMut(&FileHir, ExprId),
) {
    for &id in hir.stmt_list(statements) {
        match hir.stmt(id) {
            StmtData::If {
                then_branch,
                else_branch,
                ..
            } => {
                visit_foreach_targets(hir, *then_branch, offset, visit);
                visit_foreach_targets(hir, *else_branch, offset, visit);
            }
            StmtData::Foreach {
                key, value, body, ..
            } => {
                // The targets are bound by the loop header, so they are in scope from there on --
                // including inside the body, which is where they are almost always used.
                if hir.expr(*value).range().start() <= offset {
                    if let Some(key) = key {
                        visit(hir, *key);
                    }
                    visit(hir, *value);
                }
                visit_foreach_targets(hir, *body, offset, visit);
            }
            StmtData::While { body, .. }
            | StmtData::For { body, .. }
            | StmtData::Block { body, .. } => {
                visit_foreach_targets(hir, *body, offset, visit);
            }
            StmtData::Try {
                body,
                catches,
                finally,
                ..
            } => {
                visit_foreach_targets(hir, *body, offset, visit);
                visit_foreach_targets(hir, *finally, offset, visit);
                for &catch in hir.catch_list(*catches) {
                    visit_foreach_targets(hir, hir.catch(catch).body, offset, visit);
                }
            }
            StmtData::Switch { cases, .. } => {
                for &case in hir.switch_case_list(*cases) {
                    visit_foreach_targets(hir, hir.switch_case(case).body, offset, visit);
                }
            }
            StmtData::Expr { .. }
            | StmtData::Return { .. }
            | StmtData::Leave { .. }
            // Holds no expression and no children: it is a note about a name.
            | StmtData::Annotation { .. }
            | StmtData::Unmodelled { .. }
            // Holds no `foreach`, no `catch`, no guarding `if`, and no statements of its own.
            | StmtData::Bind { .. } => {}
        }
    }
}

/// Visits every `catch` clause under `statements`, without descending into closure bodies.
pub(crate) fn visit_catches(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    visit: &mut impl FnMut(&FileHir, phrost_hir::CatchId),
) {
    for &id in hir.stmt_list(statements) {
        match hir.stmt(id) {
            StmtData::If {
                then_branch,
                else_branch,
                ..
            } => {
                visit_catches(hir, *then_branch, visit);
                visit_catches(hir, *else_branch, visit);
            }
            StmtData::Foreach { body, .. }
            | StmtData::While { body, .. }
            | StmtData::For { body, .. }
            | StmtData::Block { body, .. } => visit_catches(hir, *body, visit),
            StmtData::Try {
                body,
                catches,
                finally,
                ..
            } => {
                visit_catches(hir, *body, visit);
                visit_catches(hir, *finally, visit);
                for &catch in hir.catch_list(*catches) {
                    visit(hir, catch);
                    visit_catches(hir, hir.catch(catch).body, visit);
                }
            }
            StmtData::Switch { cases, .. } => {
                for &case in hir.switch_case_list(*cases) {
                    visit_catches(hir, hir.switch_case(case).body, visit);
                }
            }
            StmtData::Expr { .. }
            | StmtData::Return { .. }
            | StmtData::Leave { .. }
            // Holds no expression and no children: it is a note about a name.
            | StmtData::Annotation { .. }
            | StmtData::Unmodelled { .. }
            // Holds no `foreach`, no `catch`, no guarding `if`, and no statements of its own.
            | StmtData::Bind { .. } => {}
        }
    }
}

/// Visits every expression under `statements`, without descending into closure bodies.
///
/// Stopping at a closure is the point: the caller is looking for the closures at one level, and
/// descending would find one nested two deep before the one that encloses it.
pub(crate) fn visit_statements(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    visit: &mut impl FnMut(&FileHir, ExprId),
) {
    for &id in hir.stmt_list(statements) {
        visit_one_statement(hir, id, visit);
    }
}

/// Every expression one statement holds, its nested statements included.
///
/// [`visit_statements`] over a list of one, which is the shape a reader holding a single `StmtId`
/// needs -- asking whether a name is mentioned in *this* statement is not a question a list walker
/// can answer. Exhaustive over the statement kinds, and that is load-bearing rather than tidy: a
/// reader that concludes "not mentioned here" from a walk which skips a kind is making a claim
/// about code nobody read.
pub(crate) fn visit_one_statement(
    hir: &FileHir,
    id: StmtId,
    visit: &mut impl FnMut(&FileHir, ExprId),
) {
    match hir.stmt(id) {
        StmtData::Expr { expr, .. } => visit_expression(hir, *expr, visit),
        StmtData::Return { value, .. } => {
            if let Some(value) = value {
                visit_expression(hir, *value, visit);
            }
        }
        StmtData::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            visit_expression(hir, *condition, visit);
            visit_statements(hir, *then_branch, visit);
            visit_statements(hir, *else_branch, visit);
        }
        StmtData::Foreach {
            subject,
            key,
            value,
            body,
            ..
        } => {
            visit_expression(hir, *subject, visit);
            if let Some(key) = key {
                visit_expression(hir, *key, visit);
            }
            visit_expression(hir, *value, visit);
            visit_statements(hir, *body, visit);
        }
        StmtData::While {
            condition, body, ..
        } => {
            visit_expression(hir, *condition, visit);
            visit_statements(hir, *body, visit);
        }
        // All three clauses, because each holds expressions and the first of them binds:
        // `for ($i = 0; ...)` is where `$i` comes from.
        StmtData::For {
            init,
            conditions,
            step,
            body,
            ..
        } => {
            for list in [init, conditions, step] {
                visit_all(hir, hir.expr_list(*list), visit);
            }
            visit_statements(hir, *body, visit);
        }
        StmtData::Block { body, .. } => visit_statements(hir, *body, visit),
        StmtData::Try {
            body,
            catches,
            finally,
            ..
        } => {
            visit_statements(hir, *body, visit);
            visit_statements(hir, *finally, visit);
            for &catch in hir.catch_list(*catches) {
                visit_statements(hir, hir.catch(catch).body, visit);
            }
        }
        StmtData::Switch { subject, cases, .. } => {
            visit_expression(hir, *subject, visit);
            for &case in hir.switch_case_list(*cases) {
                let case = hir.switch_case(case);
                visit_all(hir, hir.expr_list(case.conditions), visit);
                visit_statements(hir, case.body, visit);
            }
        }
        // No expressions of their own, and nothing after them to visit.
        StmtData::Leave { .. } | StmtData::Annotation { .. } => {}
        // The bindings are expressions and must be reached, or a call written in
        // `static $x = $this->build();` becomes unclickable.
        StmtData::Bind { bindings, .. } => visit_all(hir, hir.expr_list(*bindings), visit),
        // Not modelled as a statement, but the expressions inside it are still here.
        StmtData::Unmodelled { inner, .. } => visit_all(hir, hir.expr_list(*inner), visit),
    }
}

pub(crate) fn visit_expression(
    hir: &FileHir,
    id: ExprId,
    visit: &mut impl FnMut(&FileHir, ExprId),
) {
    visit(hir, id);

    // Exhaustive, with no catch-all arm, and that is the point. A shape that holds an expression
    // and does not descend into it makes everything written inside unreachable: goto, hover and
    // completion all go quiet on a call nested in it, and nothing fails, because the outer shape
    // still resolves. This has been got wrong three times -- by an unmodelled shape before it kept
    // its subtree, by a cast and an array literal, and by `&&`. A fourth has to argue with the
    // compiler.
    match hir.expr(id) {
        ExprData::Assign { target, value, .. } => {
            visit_expression(hir, *target, visit);
            visit_expression(hir, *value, visit);
        }
        ExprData::CompoundAssign { target, value, .. } => {
            visit_expression(hir, *target, visit);
            if let Some(value) = value {
                visit_expression(hir, *value, visit);
            }
        }
        ExprData::Cloned { value, .. } => visit_expression(hir, *value, visit),
        ExprData::New { class, args, .. } => {
            visit_class_target(hir, class, visit);
            visit_all(hir, hir.expr_list(*args), visit);
        }
        // The selector as well as the arguments: `$this->{$func}()` computes its name, and the
        // expression that does it holds a variable like any other.
        ExprData::StaticCall {
            class,
            args,
            selector,
            ..
        } => {
            visit_class_target(hir, class, visit);
            visit_all(hir, hir.expr_list(*args), visit);
            visit_option(hir, *selector, visit);
        }
        ExprData::FunctionCall { args, .. } => visit_all(hir, hir.expr_list(*args), visit),
        ExprData::MethodCall {
            receiver,
            args,
            selector,
            ..
        } => {
            visit_expression(hir, *receiver, visit);
            visit_all(hir, hir.expr_list(*args), visit);
            visit_option(hir, *selector, visit);
        }
        ExprData::PropertyAccess {
            receiver, selector, ..
        } => {
            visit_expression(hir, *receiver, visit);
            visit_option(hir, *selector, visit);
        }
        ExprData::StaticPropertyAccess {
            class, selector, ..
        }
        | ExprData::ClassConstAccess {
            class, selector, ..
        } => {
            visit_class_target(hir, class, visit);
            visit_option(hir, *selector, visit);
        }
        // No arguments to descend into -- a first-class callable writes none -- so the receiver and
        // the computed name are the whole of what it holds.
        ExprData::CallableMember {
            receiver, selector, ..
        } => {
            match receiver {
                phrost_hir::body::CallableReceiver::Value(value) => {
                    visit_expression(hir, *value, visit);
                }
                phrost_hir::body::CallableReceiver::Class(class) => {
                    visit_class_target(hir, class, visit);
                }
            }
            visit_option(hir, *selector, visit);
        }
        ExprData::IndirectCall { callee, args, .. } => {
            visit_expression(hir, *callee, visit);
            visit_all(hir, hir.expr_list(*args), visit);
        }
        ExprData::Instanceof { value, class, .. } => {
            visit_expression(hir, *value, visit);
            visit_class_target(hir, class, visit);
        }
        // A write through an element is still a write to the base, and both halves hold
        // expressions: `$rows[$this->key()] = $repo->find()`.
        ExprData::Index { base, index, .. } => {
            visit_expression(hir, *base, visit);
            if let Some(index) = index {
                visit_expression(hir, *index, visit);
            }
        }
        ExprData::Reference { value, .. } => visit_expression(hir, *value, visit),
        // Both of these state a type *and* contain expressions. Modelling one without descending
        // into the other is how `$x = [$repo->find(), 2]` loses the call inside it.
        ExprData::Cast { value, .. } => visit_expression(hir, *value, visit),
        ExprData::ArrayLiteral { elements, .. } => visit_all(hir, hir.expr_list(*elements), visit),
        // Both halves of a pair, in written order. A computed key holds expressions as readily as a
        // value does -- `[$this->slug() => $row]` names a method on the left of the arrow -- and it
        // was reachable before only because the flat list it arrived in did not tell the two apart.
        // A literal key has no node to visit and nothing inside it to reach.
        ExprData::ArrayElement { kind, value, .. } => {
            if let phrost_hir::body::ArrayElementKind::ComputedKey { key } = kind {
                visit_expression(hir, *key, visit);
            }
            visit_expression(hir, *value, visit);
        }
        // The wrapper is not the value: what was passed is still reachable, and a member access
        // inside a named argument resolves like any other.
        ExprData::Argument { value, .. } => visit_expression(hir, *value, visit),
        // These three are modelled for their effect on flow and on narrowing, which read the shape
        // and never the contents -- so nothing had ever needed to descend, and nothing did. Every
        // receiver written inside a guard chain was therefore unreachable, and
        // `$account->hasPermission(...) && $app instanceof T` is one of the commonest lines in a
        // Drupal access check.
        ExprData::Not { value, .. }
        | ExprData::Signed { value, .. }
        | ExprData::Throw { value, .. }
        | ExprData::NullTest { value, .. }
        | ExprData::BoolTest { value, .. } => {
            visit_expression(hir, *value, visit);
        }
        ExprData::Include { path, .. } => visit_expression(hir, *path, visit),
        ExprData::Logical { left, right, .. } | ExprData::Coalesce { left, right, .. } => {
            visit_expression(hir, *left, visit);
            visit_expression(hir, *right, visit);
        }
        ExprData::Conditional {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            visit_expression(hir, *condition, visit);
            if let Some(then_branch) = then_branch {
                visit_expression(hir, *then_branch, visit);
            }
            visit_expression(hir, *else_branch, visit);
        }
        ExprData::Match { subject, arms, .. } => {
            visit_expression(hir, *subject, visit);
            for &arm in hir.match_arm_list(*arms) {
                let arm = hir.match_arm(arm);
                visit_all(hir, hir.expr_list(arm.conditions), visit);
                visit_expression(hir, arm.body, visit);
            }
        }
        // A capture is written in the enclosing scope, so it belongs to this walk. The closure's
        // body does not: it is a scope of its own, and a caller that wants it asks for it.
        ExprData::Closure { captures, .. } => visit_all(hir, hir.expr_list(*captures), visit),
        // Not modelled, but what is written inside it is still reachable.
        ExprData::Operator { inner, .. }
        | ExprData::Terminate { inner, .. }
        | ExprData::Unmodelled { inner, .. } => visit_all(hir, hir.expr_list(*inner), visit),
        ExprData::Comparison { left, right, .. } | ExprData::Arithmetic { left, right, .. } => {
            visit_expression(hir, *left, visit);
            visit_expression(hir, *right, visit);
        }
        // Nothing is written inside these. A first-class callable naming a function writes
        // only that name, which is a reference rather than an expression.
        ExprData::Variable { .. }
        | ExprData::IndirectVariable { .. }
        | ExprData::ConstantAccess { .. }
        | ExprData::CallableFunction { .. }
        | ExprData::Literal { .. } => {}
    }
}

/// The class side of `new $factory`, `$class::make()`, `$value instanceof $class`.
///
/// A class named by a value holds an expression like any other, and a variable written there is one
/// goto has to reach.
fn visit_class_target(
    hir: &FileHir,
    target: &ClassTarget,
    visit: &mut impl FnMut(&FileHir, ExprId),
) {
    match target {
        ClassTarget::Expr(id) => visit_expression(hir, *id, visit),
        // A written name is a name reference, answered without the body.
        ClassTarget::Named(_) | ClassTarget::Relative { .. } => {}
    }
}

fn visit_option(hir: &FileHir, id: Option<ExprId>, visit: &mut impl FnMut(&FileHir, ExprId)) {
    if let Some(id) = id {
        visit_expression(hir, id, visit);
    }
}

pub(crate) fn visit_all(hir: &FileHir, ids: &[ExprId], visit: &mut impl FnMut(&FileHir, ExprId)) {
    for &id in ids {
        visit_expression(hir, id, visit);
    }
}

/// Visits every `foreach` written before `offset`, with its subject and targets.
///
/// Separate from [`visit_foreach_targets`], which only wants the names: this one needs the subject
/// too, because the target's type is the subject's element.
/// One `foreach` header, and where the loop it heads reaches.
///
/// `loop_range` is the whole statement, which is what a reader needs to tell the binding that is
/// *in scope* at a position from one that merely ran before it: PHP leaks a loop variable past its
/// closing brace, so every `foreach` written above the cursor binds the name and only the one the
/// cursor is inside is the one a person means.
#[derive(Clone, Copy)]
pub(crate) struct ForeachBinding {
    pub(crate) subject: ExprId,
    pub(crate) key: Option<ExprId>,
    pub(crate) value: ExprId,
    /// `/** @var Special $item */` written above the loop.
    pub(crate) annotated: Option<phrost_hir::TypeRefId>,
    pub(crate) loop_range: TextRange,
}

pub(crate) fn visit_foreach_bindings(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    offset: TextSize,
    visit: &mut impl FnMut(&FileHir, ForeachBinding),
) {
    for &id in hir.stmt_list(statements) {
        match hir.stmt(id) {
            // One `if` takes one arm, so a loop in the arm the position is not in bound nothing on
            // the way there. `if (str_contains($term, '+')) { foreach ($terms as $term) {...} } else
            // { preg_split('/,/', $term); }` reads the `$term` written above the `if`, and position
            // alone handed the `else` the loop's element -- the one place a binding ignored the
            // branch rule every assignment already follows.
            StmtData::If {
                then_branch,
                else_branch,
                ..
            } => {
                let holds = |branch: IdSlice<StmtId>| {
                    branch_range(hir, branch).is_some_and(|range| range.contains_inclusive(offset))
                };
                if !holds(*else_branch) {
                    visit_foreach_bindings(hir, *then_branch, offset, visit);
                }
                if !holds(*then_branch) {
                    visit_foreach_bindings(hir, *else_branch, offset, visit);
                }
            }
            StmtData::Foreach {
                subject,
                key,
                value,
                body,
                doc_type_ref,
                range,
            } => {
                // From the key when there is one, because it is written first: gating on the
                // value hid the binding from a cursor sitting on the key itself, so `foreach
                // ($items as $id => $item)` answered for `$item` and not for `$id`. Widening it
                // this far can only change positions between the two, which is the key and the
                // `=>`.
                if hir.expr(key.unwrap_or(*value)).range().start() <= offset {
                    visit(
                        hir,
                        ForeachBinding {
                            subject: *subject,
                            key: *key,
                            value: *value,
                            annotated: *doc_type_ref,
                            loop_range: *range,
                        },
                    );
                }
                visit_foreach_bindings(hir, *body, offset, visit);
            }
            StmtData::While { body, .. }
            | StmtData::For { body, .. }
            | StmtData::Block { body, .. } => {
                visit_foreach_bindings(hir, *body, offset, visit);
            }
            StmtData::Try {
                body,
                catches,
                finally,
                ..
            } => {
                visit_foreach_bindings(hir, *body, offset, visit);
                visit_foreach_bindings(hir, *finally, offset, visit);
                for &catch in hir.catch_list(*catches) {
                    visit_foreach_bindings(hir, hir.catch(catch).body, offset, visit);
                }
            }
            StmtData::Switch { cases, .. } => {
                for &case in hir.switch_case_list(*cases) {
                    visit_foreach_bindings(hir, hir.switch_case(case).body, offset, visit);
                }
            }
            StmtData::Expr { .. }
            | StmtData::Return { .. }
            | StmtData::Leave { .. }
            // Holds no expression and no children: it is a note about a name.
            | StmtData::Annotation { .. }
            | StmtData::Unmodelled { .. }
            // Holds no `foreach`, no `catch`, no guarding `if`, and no statements of its own.
            | StmtData::Bind { .. } => {}
        }
    }
}

/// Visits every `if` whose then-branch contains `offset`, innermost last.
///
/// The branch, not the statement: a narrowing holds inside the branch it guards and nowhere else.
/// Every guard written in the same expression as the position it protects, and the polarity under
/// which it holds.
///
/// `$x instanceof T && $x->m()` proves `T` for the right operand, and so does
/// `!($x instanceof T) || $x->m()`. Neither is a statement, so neither reaches
/// [`visit_guarding_ifs`] -- and a condition that uses the narrowing it has just established is how
/// both of those get written. Drupal-shaped code is full of the first:
/// `if ($app instanceof BulkOrderApplication && $app->getUnpaidBalance() > 0)`.
///
/// Containment is by text range, so a guard whose right operand holds a closure still covers a
/// position inside that closure's body -- which is right, because an arrow function captures the
/// narrowed value.
#[cfg(test)]
pub(crate) fn visit_guarding_operands(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    offset: TextSize,
    visit: &mut impl FnMut(&FileHir, ExprId, bool),
) {
    visit_statements(hir, statements, &mut |hir, expr| {
        match hir.expr(expr) {
            ExprData::Logical {
                operator,
                left,
                right,
                ..
            } if hir.expr(*right).range().contains_inclusive(offset) => {
                // `&&` carries the claim as written; `||` carries its negation, which is the same
                // rule `asserted_class` already applies when it walks through the operators.
                visit(hir, *left, matches!(operator, LogicalOp::And));
            }
            // A ternary selects an arm by its condition, so the condition holds in that arm -- the
            // then-branch as written, the else-branch negated. `?:` has no then-branch to be in.
            ExprData::Conditional {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if then_branch.is_some_and(|then| hir.expr(then).range().contains_inclusive(offset))
                {
                    visit(hir, *condition, true);
                } else if hir.expr(*else_branch).range().contains_inclusive(offset) {
                    visit(hir, *condition, false);
                }
            }
            // `match (true)` is an `if`/`elseif` chain written as an expression: an arm runs when
            // its own condition matched and every earlier one did not. Only where the subject is
            // the literal, since any other subject compares values rather than testing conditions.
            ExprData::Match { subject, arms, .. } => {
                let Some(matched) = literal_bool(hir, *subject) else {
                    return;
                };
                let list = hir.match_arm_list(*arms);
                let Some(index) = list.iter().position(|&arm| {
                    hir.expr(hir.match_arm(arm).body)
                        .range()
                        .contains_inclusive(offset)
                }) else {
                    return;
                };
                for &earlier in &list[..index] {
                    for &condition in hir.expr_list(hir.match_arm(earlier).conditions) {
                        visit(hir, condition, !matched);
                    }
                }
                // An arm listing two conditions proves nothing of its own: either may be the one
                // that matched.
                if let [only] = hir.expr_list(hir.match_arm(list[index]).conditions) {
                    visit(hir, *only, matched);
                }
            }
            _ => {}
        }
    });
}

/// The `true` or `false` a `match` subject was written as, and nothing else.
#[cfg(test)]
fn literal_bool(hir: &FileHir, subject: ExprId) -> Option<bool> {
    match hir.expr(subject) {
        ExprData::Literal {
            kind: phrost_hir::body::LiteralKind::True,
            ..
        } => Some(true),
        ExprData::Literal {
            kind: phrost_hir::body::LiteralKind::False,
            ..
        } => Some(false),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn visit_guarding_ifs(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    offset: TextSize,
    visit: &mut impl FnMut(&FileHir, ExprId, TextRange, bool),
) {
    for &id in hir.stmt_list(statements) {
        match hir.stmt(id) {
            StmtData::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(range) = branch_range(hir, *then_branch)
                    && range.contains_inclusive(offset)
                {
                    visit(hir, *condition, range, true);
                    visit_guarding_ifs(hir, *then_branch, offset, visit);
                }
                // The else branch is guarded by the negation of the same condition, which is as much
                // of a claim as the then branch is: after `if (!$app) { } else { ... }` the value is
                // not null in there. Only the then branch used to be offered, so half of every
                // written-out guard proved nothing.
                if let Some(range) = branch_range(hir, *else_branch)
                    && range.contains_inclusive(offset)
                {
                    visit(hir, *condition, range, false);
                }
                visit_guarding_ifs(hir, *else_branch, offset, visit);
            }
            // A loop's condition is a guard over its own body, and the commonest one written in PHP:
            // `while ($row = fgetcsv($handle))` and `while (($row = fgetcsv($handle)) !== FALSE)`
            // are how a file is read, and inside the body the row is there. The same words after a
            // `do` prove it only from the second turn onwards, so `tested_first` decides -- which is
            // why the two are told apart at all.
            StmtData::While {
                condition,
                body,
                tested_first: true,
                ..
            } => {
                if let Some(range) = branch_range(hir, *body)
                    && range.contains_inclusive(offset)
                {
                    visit(hir, *condition, range, true);
                }
                visit_guarding_ifs(hir, *body, offset, visit);
            }
            // The same claim, from the clause that decides whether the body runs: the *last* of
            // them, since PHP evaluates a comma-separated list and takes the value of the last.
            StmtData::For {
                conditions, body, ..
            } => {
                if let Some(&last) = hir.expr_list(*conditions).last()
                    && let Some(range) = branch_range(hir, *body)
                    && range.contains_inclusive(offset)
                {
                    visit(hir, last, range, true);
                }
                visit_guarding_ifs(hir, *body, offset, visit);
            }
            StmtData::Foreach { body, .. }
            | StmtData::While { body, .. }
            | StmtData::Block { body, .. } => visit_guarding_ifs(hir, *body, offset, visit),
            StmtData::Try {
                body,
                catches,
                finally,
                ..
            } => {
                visit_guarding_ifs(hir, *body, offset, visit);
                visit_guarding_ifs(hir, *finally, offset, visit);
                for &catch in hir.catch_list(*catches) {
                    visit_guarding_ifs(hir, hir.catch(catch).body, offset, visit);
                }
            }
            StmtData::Switch { cases, .. } => {
                for &case in hir.switch_case_list(*cases) {
                    visit_guarding_ifs(hir, hir.switch_case(case).body, offset, visit);
                }
            }
            StmtData::Expr { .. }
            | StmtData::Return { .. }
            | StmtData::Leave { .. }
            // Holds no expression and no children: it is a note about a name.
            | StmtData::Annotation { .. }
            | StmtData::Unmodelled { .. }
            // Holds no `foreach`, no `catch`, no guarding `if`, and no statements of its own.
            | StmtData::Bind { .. } => {}
        }
    }
}

/// The span a branch's statements cover, or `None` for an empty branch.
pub(crate) fn branch_range(hir: &FileHir, statements: IdSlice<StmtId>) -> Option<TextRange> {
    let list = hir.stmt_list(statements);
    let first = list.first()?;
    let last = list.last()?;
    Some(TextRange::new(
        hir.stmt(*first).range().start(),
        hir.stmt(*last).range().end(),
    ))
}

/// Whether every path through `statements` leaves before reaching the end.
///
/// What makes an early return narrow the code after it: `if (!($x instanceof T)) { return; }` proves
/// the negation *only* because the branch cannot fall through. Conservative -- an unmodelled
/// statement is assumed to fall through, so the answer is only ever "yes, definitely".
pub(crate) fn always_leaves(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    never: NeverReturns<'_>,
) -> bool {
    leaves(hir, statements, true, never)
}

/// Whether a call leaves the function for good, asked of whoever can look the callee up.
///
/// `return`, `throw`, `exit` and `die` are syntax and are decided here. A call to something declared
/// `: never` is not: what it returns is written in whatever file declares it, and this layer reads
/// one. So the question goes back to the reader -- the same shape [`not_null_checks`] uses for an
/// contract tag, and for the same reason.
///
/// [`not_null_checks`]: crate::not_null_checks
pub type NeverReturns<'a> = &'a dyn Fn(&FileHir, ExprId) -> bool;

/// Whether a *function* of this written name carries an assertion about one of its arguments,
/// asked of whoever can look the declaration up.
///
/// The same shape as [`NeverReturns`] and for the same reason: `if (f($x))` is one of the commonest
/// things a PHP file contains, what `f` promises is written in whatever file declares it, and a
/// narrowing root built for a callee that turns out to promise nothing costs precision rather than
/// only time -- it resolves its subject at the guard, so it answers with what the value held
/// *before* the branch. So the question is put back to the reader, which answers it from a
/// project-wide name set before resolving anything.
///
/// Threaded beside `never` rather than bundled with it, because the two have almost disjoint
/// consumers: `never` is asked by the flow, reach and facts layers, which have no guards in hand,
/// and this is asked only on the guard path. The third question about a guard arrived -- what it
/// already proved, remembered -- and it rides here, on the guard path, as [`Asserts::memo`]: a plain
/// closure still answers the first question and keeps nothing.
///
/// [`Asserts::memo`]: crate::proved_memo::Asserts::memo
pub type AssertsArguments<'a> = &'a dyn crate::proved_memo::Asserts;

/// `break_escapes` says whether a `break` here leaves the region being asked about.
///
/// It does inside a loop's body, which is where this is asked from: after `if (!$c) { break; }` the
/// rest of that iteration only runs when the condition held. It does not inside a `switch` arm,
/// where a `break` is consumed by the switch and execution continues after it -- so a switch whose
/// every arm ends in `break` leaves nothing at all, and counting it as leaving would let a narrowing
/// claim something about code that is plainly reachable.
pub(crate) fn leaves(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    break_escapes: bool,
    never: NeverReturns<'_>,
) -> bool {
    // Any statement, not only the last. `abort(); $x = 1;` is a block nothing falls out of: the
    // assignment after the call is dead code, and reading the *last* statement made it evidence
    // that the block continues -- so a guard whose branch was written that way narrowed nothing.
    hir.stmt_list(statements)
        .iter()
        .any(|&statement| statement_leaves(hir, statement, break_escapes, never))
}

/// Whether one statement leaves the list it is written in, on every path through it.
pub(crate) fn statement_leaves(
    hir: &FileHir,
    statement: StmtId,
    break_escapes: bool,
    never: NeverReturns<'_>,
) -> bool {
    match hir.stmt(statement) {
        StmtData::Return { .. } => true,
        StmtData::Leave { .. } => break_escapes,
        StmtData::Expr { expr, .. } => match hir.expr(*expr) {
            ExprData::Throw { .. } | ExprData::Terminate { .. } => true,
            // `$this->fail();` where `fail(): never`. Written exactly like a call that returns, so
            // the declaration is the only thing that says otherwise.
            _ => never(hir, *expr),
        },
        StmtData::Block { body, .. } => leaves(hir, *body, break_escapes, never),
        // Both branches, and both non-empty: an `if` with no else falls through when the condition
        // is false.
        StmtData::If {
            then_branch,
            else_branch,
            ..
        } => {
            leaves(hir, *then_branch, break_escapes, never)
                && leaves(hir, *else_branch, break_escapes, never)
        }
        // Exhaustive only with a `default` arm -- without one, a value matching no case falls
        // through the whole statement. And every arm has to leave, including the default: one
        // that falls off the end makes the statement as a whole merely "maybe".
        StmtData::Switch { cases, .. } => {
            let cases = hir.switch_case_list(*cases);
            cases.iter().any(|&case| hir.switch_case(case).is_default)
                && cases
                    .iter()
                    // `false`: a `break` in an arm is consumed by the switch, so it leaves the arm
                    // and not the function.
                    .all(|&case| leaves(hir, hir.switch_case(case).body, false, never))
        }
        // A loop may run zero times, a `try` may not throw, and anything unmodelled is assumed to
        // continue. Each of those makes this "no", which is the safe direction.
        _ => false,
    }
}

/// Whether a bare call statement could be the thing that leaves.
///
/// The optimistic oracle: `true` for every call, so [`leaves`] asked with it answers "yes" wherever
/// a `: never` declaration *could* make it so. Asked beside the pessimistic one, the two answers say
/// whether the question is worth putting to a reader that can look the callee up.
pub(crate) fn any_call_leaves(hir: &FileHir, expr: ExprId) -> bool {
    is_call(hir, expr)
}

/// Visits the statements written before `offset` in every list that encloses it, outermost first.
///
/// What an early return needs: the guard is a *sibling* of the code it narrows, not its parent, so
/// the guarding-if walk cannot see it.
/// Every statement before `offset`, descending into a compound statement that has already closed.
///
/// The difference from [`visit_preceding_statements`] is what a *finished* branch means, and it is a
/// real difference. A guard is a claim about the code that follows it, so a branch that closed says
/// nothing there; a write into an array is a claim about the array, and a slot filled inside a
/// finished `if` or `foreach` is still a slot that was filled.
pub(crate) fn visit_statements_before(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    offset: TextSize,
    visit: &mut impl FnMut(&FileHir, StmtId),
) {
    for &id in hir.stmt_list(statements) {
        let data = hir.stmt(id);
        // Closed statements are visited; the one the offset is inside is only descended into. A
        // statement cannot describe a value read inside itself --
        // `$v['x'] = AddressType::tryFrom($v['x'])` reads the slot it is about to write, and visiting
        // it here answered that read with its own result.
        let closed = data.range().end() <= offset;
        if !closed && !data.range().contains_inclusive(offset) {
            continue;
        }
        if closed {
            visit(hir, id);
        }
        for nested in [match data {
            StmtData::If {
                then_branch,
                else_branch,
                ..
            } => [Some(*then_branch), Some(*else_branch)],
            StmtData::Foreach { body, .. }
            | StmtData::While { body, .. }
            | StmtData::For { body, .. }
            | StmtData::Block { body, .. }
            | StmtData::Try { body, .. } => [Some(*body), None],
            _ => [None, None],
        }]
        .into_iter()
        .flatten()
        .flatten()
        {
            visit_statements_before(hir, nested, offset, visit);
        }
    }
}

/// Every call expression that certainly ran before `offset`, and whether it was a statement of its
/// own.
///
/// Its own traversal rather than a widening of [`visit_preceding_statements`], which several guard
/// readers share and which answers a different question: that one hands over *statements*, and a
/// guard's meaning depends on which branch it is written in. This one hands over *calls*, because a
/// by-reference parameter writes to the caller's variable wherever the call was written -- and the
/// four ways one function is written are not four statements:
///
/// ```php
/// preg_match($p, $s, $m);          // a statement of its own
/// $ok = preg_match($p, $s, $m);    // inside an assignment
/// if (preg_match($p, $s, $m)) {}   // inside the condition of the statement the cursor is in
/// ```
///
/// Only what *ran*. A fully preceding statement ran, so every call inside it did; the condition of
/// the statement the offset sits inside ran, because a body is only reached through it; and a sibling
/// branch did not, which is why this descends the way it does rather than visiting everything before
/// the offset by position.
///
/// The flag is for a reader that needs the stricter shape. An `@assert` tag is written on a call made
/// for its effect and nothing else -- `$this->assertNotNull($x);` -- and one inside a condition proves
/// nothing about the code after it, so that reader keeps requiring a statement while the
/// by-reference reader does not.
pub(crate) fn visit_calls_before(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    offset: TextSize,
    visit: &mut impl FnMut(&FileHir, ExprId, bool),
) {
    for &id in hir.stmt_list(statements) {
        let data = hir.stmt(id);
        if data.range().end() <= offset {
            // Wholly before the offset, so everything written in it ran. The statement's own
            // expression is the one a contract can be written about.
            if let StmtData::Expr { expr, .. } = data {
                visit(hir, *expr, true);
            }
            // Its own expressions *and* its condition. A closed `if` is wholly behind the offset, so
            // its condition ran as surely as its body did -- and leaving it out was the one shape of
            // eleven that still answered nothing: `if (preg_match($p, $s, $m)) {} $m`.
            for expr in statement_expressions(data)
                .into_iter()
                .chain(statement_conditions(hir, data))
            {
                visit_expression(hir, expr, &mut |hir, inner| {
                    if is_call(hir, inner) {
                        visit(hir, inner, false);
                    }
                });
            }
            // A statement wholly before the offset may still hold statements of its own -- a loop
            // body, an `if` that closed -- and those ran too.
            for list in statement_bodies(hir, data) {
                visit_calls_before(hir, list, offset, visit);
            }
            continue;
        }
        if !data.range().contains_inclusive(offset) {
            continue;
        }
        // The statement the offset is inside. Its *condition* ran, because that is how the body was
        // reached, and then whichever of its lists holds the offset.
        for expr in statement_conditions(hir, data) {
            visit_expression(hir, expr, &mut |hir, inner| {
                if is_call(hir, inner) {
                    visit(hir, inner, false);
                }
            });
        }
        for list in statement_bodies(hir, data) {
            visit_calls_before(hir, list, offset, visit);
        }
    }
}

/// Every statement list a statement holds.
///
/// Written out rather than derived from the node's children, because the point is to walk *only*
/// statements: descending through expressions here would reach a closure body, whose calls have not
/// run at all.
fn statement_bodies(hir: &FileHir, data: &StmtData) -> Vec<IdSlice<StmtId>> {
    match data {
        StmtData::If {
            then_branch,
            else_branch,
            ..
        } => vec![*then_branch, *else_branch],
        StmtData::Foreach { body, .. }
        | StmtData::While { body, .. }
        | StmtData::For { body, .. }
        | StmtData::Block { body, .. } => vec![*body],
        StmtData::Try {
            body,
            catches,
            finally,
            ..
        } => {
            let mut lists = vec![*body, *finally];
            lists.extend(
                hir.catch_list(*catches)
                    .iter()
                    .map(|&catch| hir.catch(catch).body),
            );
            lists
        }
        StmtData::Switch { cases, .. } => hir
            .switch_case_list(*cases)
            .iter()
            .map(|&case| hir.switch_case(case).body)
            .collect(),
        _ => Vec::new(),
    }
}

/// Whether the expression is a call of any of the four shapes a callee can be named by.
fn is_call(hir: &FileHir, expr: ExprId) -> bool {
    matches!(
        hir.expr(expr),
        ExprData::MethodCall { .. }
            | ExprData::StaticCall { .. }
            | ExprData::FunctionCall { .. }
            | ExprData::New { .. }
    )
}

/// The expressions a statement evaluates itself, excluding its nested statement lists.
fn statement_expressions(data: &StmtData) -> Vec<ExprId> {
    match data {
        StmtData::Expr { expr, .. } => vec![*expr],
        StmtData::Return { value, .. } => value.iter().copied().collect(),
        _ => Vec::new(),
    }
}

/// The expressions a statement evaluates before reaching its body.
///
/// A `foreach`'s subject and a `switch`'s subject count: both are evaluated once, before anything in
/// the body. A `for`'s conditions are re-evaluated per iteration, which does not change that they ran
/// at least once by the time the body is reached.
fn statement_conditions(hir: &FileHir, data: &StmtData) -> Vec<ExprId> {
    match data {
        StmtData::If { condition, .. } | StmtData::While { condition, .. } => vec![*condition],
        StmtData::For { conditions, .. } => hir.expr_list(*conditions).to_vec(),
        StmtData::Foreach { subject, .. } => vec![*subject],
        StmtData::Switch { subject, .. } => vec![*subject],
        _ => Vec::new(),
    }
}

/// Every statement that finished before `offset`, plus the enclosing ones the offset sits inside.
///
/// # A finished statement can still hold statements that ran
///
/// This used to visit a finished statement and stop there, which is right for an `if` -- a guard in a
/// branch says nothing once the branch is behind you, because the branch may not have been taken. It
/// is wrong for the two kinds where *arriving here proves the inner statements ran*, and the omission
/// showed up as a `possibly-null-receiver` on code that plainly checks for null:
///
/// ```php
/// try {
///     $product = $supplier->product();
///     if ($product === NULL) {
///         throw new NoItemException('none');
///     }
/// }
/// catch (NoItemException) {
///     return $this->addError(...);
/// }
/// $product->getMinItemAmount($score);
/// ```
///
/// The guard is found while the offset is *inside* the `try`, and was lost the moment it left --
/// while the **assignment** in the same block was still visible, because that walk descends into
/// blocks. One value, two searches, and only one of them looking inside.
///
/// A `try` body earns the descent only when **no catch clause can complete normally**: a catch that
/// falls through reaches the code after the statement without the guard having run, and believing the
/// guard there would claim a type the code does not have. A `finally` earns it unconditionally,
/// because it runs on every path that reaches the `try` at all -- if it completed, it completed. A
/// bare `{ ... }` block earns it for the same reason: nothing can skip it.
///
/// An `if` earns it when one arm always leaves: the other is then the only way past, so it ran.
/// `if ($c) { return; } elseif (!$b = $this->find()) { return; }` is the shape -- an `elseif` is an
/// `if` inside the `else`, and its own leaving guard is the whole of what says `$b` is there after
/// the chain. Which arm leaves is the one thing asked; neither arm is descended into otherwise,
/// because then either may not have run.
///
/// A loop is deliberately excluded even though it looks similar. It may run zero times, so a guard in
/// its body proves nothing about the code after it.
pub(crate) fn visit_preceding_statements(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    offset: TextSize,
    never: NeverReturns<'_>,
    visit: &mut impl FnMut(&FileHir, StmtId),
) {
    for &id in hir.stmt_list(statements) {
        let data = hir.stmt(id);
        if data.range().end() <= offset {
            visit(hir, id);
            match data {
                StmtData::Block { body, .. } => {
                    visit_preceding_statements(hir, *body, offset, never, visit);
                }
                StmtData::Try {
                    body,
                    catches,
                    finally,
                    ..
                } => {
                    visit_preceding_statements(hir, *finally, offset, never, visit);
                    if hir
                        .catch_list(*catches)
                        .iter()
                        .all(|&catch| always_leaves(hir, hir.catch(catch).body, never))
                    {
                        visit_preceding_statements(hir, *body, offset, never, visit);
                    }
                }
                StmtData::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    if always_leaves(hir, *then_branch, never) {
                        visit_preceding_statements(hir, *else_branch, offset, never, visit);
                    } else if always_leaves(hir, *else_branch, never) {
                        visit_preceding_statements(hir, *then_branch, offset, never, visit);
                    }
                }
                _ => {}
            }
            continue;
        }
        if !data.range().contains_inclusive(offset) {
            continue;
        }
        // The statement the offset is inside: descend into whichever of its lists holds it.
        match data {
            StmtData::If {
                then_branch,
                else_branch,
                ..
            } => {
                // Whichever branch the offset is in, and never both: the branches of an `if` are
                // alternatives, so a statement in the one not taken did not run however plainly it
                // is written before the offset. Descending into both put a leaving guard's claim
                // into the *sibling* branch -- `if (is_array($m)) { if (!is_int($m)) { throw; } }
                // elseif (...) { $m }` proved `$m` an int on a path that never ran either line --
                // and a guard whose branch leaves narrows its siblings, which the branch's own list
                // is and the chain's other arms are not.
                //
                // The else branch's first statement is the boundary: an offset before it is in the
                // then branch or in the condition, where the recursion finds nothing either way.
                // An `elseif` is one `If` statement in that list, so the same test applies again a
                // level in.
                match hir
                    .stmt_list(*else_branch)
                    .first()
                    .map(|&first| hir.stmt(first).range().start())
                {
                    Some(otherwise) if offset >= otherwise => {
                        visit_preceding_statements(hir, *else_branch, offset, never, visit);
                    }
                    _ => visit_preceding_statements(hir, *then_branch, offset, never, visit),
                }
            }
            StmtData::Foreach { body, .. }
            | StmtData::While { body, .. }
            | StmtData::For { body, .. }
            | StmtData::Block { body, .. } => {
                visit_preceding_statements(hir, *body, offset, never, visit);
            }
            StmtData::Try {
                body,
                catches,
                finally,
                ..
            } => {
                visit_preceding_statements(hir, *body, offset, never, visit);
                visit_preceding_statements(hir, *finally, offset, never, visit);
                for &catch in hir.catch_list(*catches) {
                    visit_preceding_statements(hir, hir.catch(catch).body, offset, never, visit);
                }
            }
            StmtData::Switch { cases, .. } => {
                for &case in hir.switch_case_list(*cases) {
                    visit_preceding_statements(
                        hir,
                        hir.switch_case(case).body,
                        offset,
                        never,
                        visit,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Every statement in a body, descending into blocks but not into nested bodies.
///
/// A closure has a body of its own and its own scope, so its statements are not these -- which is
/// what makes this safe for anything asking "what does this scope say about a name".
pub fn visit_every_statement(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    visit: &mut impl FnMut(&FileHir, StmtId),
) {
    for &id in hir.stmt_list(statements) {
        visit(hir, id);

        match hir.stmt(id) {
            StmtData::If {
                then_branch,
                else_branch,
                ..
            } => {
                visit_every_statement(hir, *then_branch, visit);
                visit_every_statement(hir, *else_branch, visit);
            }
            StmtData::Foreach { body, .. }
            | StmtData::While { body, .. }
            | StmtData::For { body, .. }
            | StmtData::Block { body, .. } => visit_every_statement(hir, *body, visit),
            StmtData::Try {
                body,
                catches,
                finally,
                ..
            } => {
                visit_every_statement(hir, *body, visit);
                visit_every_statement(hir, *finally, visit);
                for &catch in hir.catch_list(*catches) {
                    visit_every_statement(hir, hir.catch(catch).body, visit);
                }
            }
            StmtData::Switch { cases, .. } => {
                for &case in hir.switch_case_list(*cases) {
                    visit_every_statement(hir, hir.switch_case(case).body, visit);
                }
            }
            StmtData::Expr { .. }
            | StmtData::Return { .. }
            | StmtData::Leave { .. }
            | StmtData::Bind { .. }
            | StmtData::Annotation { .. }
            | StmtData::Unmodelled { .. } => {}
        }
    }
}
