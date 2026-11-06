//! An expression that could be lifted into a variable of its own.
//!
//! The half of "extract variable" a body can answer: *which* expression, *where* the statement
//! holding it begins, and *whether* lifting it out is the same program. What the variable should be
//! called and what the edit looks like belong further up.
//!
//! # Why most of this is refusals
//!
//! Hoisting an expression makes it run **earlier** and **unconditionally**, and those are the two
//! ways the same characters mean something else afterwards:
//!
//! - **A short-circuit.** `$a && f()` runs `f()` only when `$a` is truthy, and `$a ?? f()` only when
//!   `$a` is null. Lifting either out runs it always -- which is a different program whenever the
//!   guard was there for a reason, and the guard is almost always there for a reason.
//! - **A branch.** The arms of `?:` and of `match` are chosen, so one of them is code that did not
//!   run.
//! - **A closure.** Its body runs when the closure is called, and the statement is outside it.
//!   Hoisting moves the code out of the function it was written in.
//!
//! Each is decided by *containment* rather than by walking a path: an expression is refused when a
//! conditional node in the same statement holds it and is not it. That needs no second traversal
//! shape and cannot miss a nesting level.
//!
//! What is deliberately **not** checked is whether the expression has side effects. Running
//! `$this->save()` one statement earlier is a change a person asked for by selecting it, and a
//! refactor that refused everything impure would refuse most of the calls anybody extracts. The
//! order within the statement is what is preserved; the order between statements is what the person
//! is choosing to change.

use phrost_hir::{
    ExprId, FileHir,
    body::{ExprData, StmtData},
};
use phrost_model::{TextRange, TextSize};

use crate::{scope::Scope, walk::visit_every_statement};

/// An expression worth offering to lift out, and what lifting it needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Extractable {
    /// The expression's own range, which is what the variable replaces.
    pub range: TextRange,
    /// Where the statement holding it begins, which is where the assignment goes above it.
    pub statement: TextSize,
    /// A name the expression suggests: the method a call names, the property an access reads.
    ///
    /// Text rather than an interned name: it is a name being *made up*, not one the project holds,
    /// and putting it in the interner would put every extraction anybody ever considers there.
    ///
    /// `None` for one that suggests nothing, where the caller has to invent a name. Taken from the
    /// expression rather than from its type, because a type is a cross-file question and this is
    /// answerable from the statement -- and because `$repository` is a worse name than `$find`
    /// for `$repo->find()` anyway.
    pub named_after: Option<Box<str>>,
}

/// The expression at `at` that could become a variable, if one can.
///
/// `at` is what the editor selected, empty for a bare cursor. The innermost expression *containing*
/// it is the answer, which for a selection covering a whole expression is that expression and for a
/// cursor is the smallest thing under it.
pub fn extractable_at(hir: &FileHir, at: TextRange) -> Option<Extractable> {
    // The innermost scope, which for a cursor inside a closure is the closure's own body: its
    // statements are the frame the variable would go into, and the closure enclosing them is then
    // outside the statement rather than around the expression.
    // The **last** of the chain, which is the innermost: `chain_at` pushes the declaration first
    // and then each closure it descends into. For a cursor inside a closure the innermost body is
    // the frame the variable goes into -- hoisting to the statement outside would move the code out
    // of the function it was written in.
    let scope = Scope::chain_at(hir, at.start());
    let scope = scope.last()?;
    // An arrow function's body is an *expression*, so there is nowhere in it to put a statement.
    // Hoisting to the statement outside would move the code out of the function it was written in,
    // which the closure rule below already refuses -- and writing it in place produced
    // `fn ($key) => $value = !empty(...); $value)`, a file PHP will not parse.
    if matches!(scope.kind(), crate::scope::ScopeKind::ArrowFunction) {
        return None;
    }

    let mut statement = None;
    visit_every_statement(hir, hir.body(scope.body()).statements, &mut |hir, id| {
        let range = hir.stmt(id).range();
        if range.contains_range(at)
            && statement.is_none_or(|held: (TextRange, _)| range.len() < held.0.len())
        {
            statement = Some((range, id));
        }
    });
    let (statement, statement_id) = statement?;
    // `elseif` is not a statement boundary. Lowering turns each clause into an `if` of its own whose
    // range begins at the keyword -- see `clause_range` -- so the assignment lands between the
    // previous arm's `}` and the `elseif`, and PHP refuses the file. The only other place it could
    // go is above the whole chain, where it would run on the path the chain exists to keep it off.
    if is_an_elseif(hir, statement_id, statement) {
        return None;
    }
    // A declaration is not code that runs here. Its defaults, its constants and its attributes are
    // lowered as nested expressions of this statement so goto and hover reach them, and lifting one
    // into a variable above the declaration writes a constant expression that names a variable --
    // `$value = ['a' => 1]; class Other { public array $shape = $value; }`, which PHP refuses. Four
    // shapes reached it: a property default, a class constant, a parameter default and an
    // attribute's argument.
    if matches!(
        hir.stmt(statement_id),
        phrost_hir::body::StmtData::Unmodelled {
            kind: phrost_hir::body::UnmodelledKind::Declaration,
            ..
        }
    ) {
        return None;
    }

    let mut inside: Vec<(TextRange, ExprId)> = Vec::new();
    let mut conditional = Vec::new();
    let mut written = Vec::new();
    // The expressions **this statement holds**, and not every expression in the file whose range
    // happens to fall inside it. The two differ exactly where a declaration is involved: a property
    // default, a class constant, a parameter default and an attribute's argument all sit inside the
    // range of the declaration statement around them and belong to none of its code. Reading them
    // by containment offered `$value = ['a' => 1]; class Other { public array $shape = $value; }`,
    // which is a constant expression referring to a variable -- four shapes, one wrong question.
    crate::walk::visit_one_statement(hir, statement_id, &mut |hir, expr| {
        let data = hir.expr(expr);
        let range = data.range();
        // A write target is a *place* and not a value: `$rows[] = $item` names where the item goes,
        // and `$value = $rows[];` is not even a thing PHP can read. Its own list rather than a
        // refusal here, because an expression written *inside* a target -- the `$i` of `$a[$i]` --
        // is an ordinary value and stays offerable.
        if let ExprData::Assign { target, .. } | ExprData::CompoundAssign { target, .. } = data {
            written.push(hir.expr(*target).range());
        }
        if !range.contains_range(at) {
            return;
        }
        if selects_or_defers(data) {
            conditional.push(range);
        }
        inside.push((range, expr));
    });

    // Innermost first, and the first of those that is worth a name of its own: a cursor on `$repo`
    // in `$repo->find()` is on a variable, which already is one, and the call around it is what a
    // person meant.
    inside.sort_by_key(|(range, _)| range.len());
    let (range, expr) = inside.into_iter().find(|(range, expr)| {
        // Where a target begins is where every prefix of it begins -- `$a`, `$a[$i]`, `$a[$i][0]`
        // -- and those are the ones that are the place rather than a value inside it.
        worth_lifting(hir.expr(*expr))
            && !written.iter().any(|target| target.start() == range.start())
    })?;
    if conditional.iter().any(|held| *held != range) {
        return None;
    }
    Some(Extractable {
        range,
        statement: statement.start(),
        named_after: suggested_by(hir, expr),
    })
}

/// Whether this statement is the `if` an `elseif` clause was lowered into.
///
/// Told apart from a real `else { if (...) {...} }` -- which *is* a place a statement can go -- by
/// where it ends: a synthesized clause owns everything after it including the final `else`, so its
/// range ends with the whole chain, while a nested `if` written inside an `else` block ends before
/// its own closing brace.
fn is_an_elseif(hir: &FileHir, statement: phrost_hir::StmtId, range: TextRange) -> bool {
    if !matches!(hir.stmt(statement), StmtData::If { .. }) {
        return false;
    }
    let mut found = false;
    for scope in Scope::every_scope(hir) {
        visit_every_statement(hir, hir.body(scope.body()).statements, &mut |hir, id| {
            let StmtData::If {
                else_branch,
                range: whole,
                ..
            } = hir.stmt(id)
            else {
                return;
            };
            if range.end() == whole.end() && hir.stmt_list(*else_branch).contains(&statement) {
                found = true;
            }
        });
    }
    found
}

/// Whether an expression chooses between values, or defers when they are produced.
fn selects_or_defers(data: &ExprData) -> bool {
    matches!(
        data,
        ExprData::Logical { .. }
            | ExprData::Coalesce { .. }
            | ExprData::Conditional { .. }
            | ExprData::Match { .. }
            | ExprData::Closure { .. }
    )
}

/// Whether an expression is worth offering as a variable of its own.
///
/// A variable already is one, and `$v = $x;` says nothing. A literal is already as plain as it gets.
/// Everything else -- a call, an access, a construction, an operator -- is something a reader could
/// want a name for.
fn worth_lifting(data: &ExprData) -> bool {
    !matches!(
        data,
        ExprData::Variable { .. }
            | ExprData::Literal { .. }
            | ExprData::ArrayElement { .. }
            // `...$rest` and `id: 1` are syntax of the *call*, not values: `$value = ...$rest;` is
            // not something PHP can read, and the expression inside either one is still offered.
            | ExprData::Argument { .. }
    )
}

/// The name the expression itself suggests.
fn suggested_by(hir: &FileHir, expr: ExprId) -> Option<Box<str>> {
    match hir.expr(expr) {
        ExprData::MethodCall { name, .. } | ExprData::StaticCall { name, .. } => {
            name.as_ref().map(|name| name.as_str().into())
        }
        ExprData::PropertyAccess { name, .. } => name.as_ref().map(|name| name.as_str().into()),
        // `new Node()` names a node. The last segment only: `new Drupal\\node\\Entity\\Node()`
        // suggests `$node` and not the namespace it lives in.
        ExprData::New {
            class: phrost_hir::body::ClassTarget::Named(id),
            ..
        } => Some(phrost_model::short_name(hir.name_ref(*id).path.as_str()).into()),
        _ => None,
    }
}

/// The literal at `at`, when the position is on one.
///
/// A string or a number only. `true`, `false` and `null` are already as plain as a name can make
/// them, and a constant standing for one says less than the word does.
pub fn literal_at(hir: &FileHir, at: TextRange) -> Option<TextRange> {
    // The scope is asked for only to refuse a position outside every body: a literal written as a
    // property's default or a class constant's value has no statement to put a constant above, and
    // is one already.
    Scope::chain_at(hir, at.start()).last()?;

    let mut found: Option<TextRange> = None;
    Scope::visit_every_expression(hir, &mut |hir, expr| {
        let ExprData::Literal { kind, range } = hir.expr(expr) else {
            return;
        };
        if !matches!(
            kind,
            phrost_hir::body::LiteralKind::String
                | phrost_hir::body::LiteralKind::Int
                | phrost_hir::body::LiteralKind::Float
        ) {
            return;
        }
        if range.contains_range(at) && found.is_none_or(|held| range.len() < held.len()) {
            found = Some(*range);
        }
    });
    found
}
