//! What a written target binds.
//!
//! PHP binds a local by being written to, and being written to is not the same as being written:
//! `$rows['a'] = 1` creates `$rows`, and `foreach ($items as &$item)` binds `$item` through a
//! reference. Both used to bind nothing, because both places asked whether the target *was* a
//! variable rather than which variable it wrote through.

use phrost_hir::{
    ExprId, FileHir,
    body::{ArrayElementKind, ExprData},
};
use phrost_model::Name;

/// The local a target writes to, and how much of it the write settles.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Bound<'a> {
    /// `$x = v`, `foreach (.. as $x)`, `foreach (.. as &$x)` — the variable holds the value.
    Whole(&'a Name),
    /// `$x['k'] = v`, `$x[] = v` — the value went into a slot. The variable is created by the
    /// write, and what it holds is an array whatever the value was.
    Element(&'a Name),
}

impl<'a> Bound<'a> {
    pub(crate) fn name(self) -> &'a Name {
        match self {
            Self::Whole(name) | Self::Element(name) => name,
        }
    }
}

/// The local `target` writes to, or `None` when it writes to something that is not one.
pub(crate) fn bound_by(hir: &FileHir, target: ExprId) -> Option<Bound<'_>> {
    match hir.expr(target) {
        ExprData::Variable { name, .. } => Some(Bound::Whole(name)),
        // The reference wraps what is bound: `&$item` binds `$item`.
        ExprData::Reference { value, .. } => bound_by(hir, *value),
        // Through a variable only. `$this->rows['a'] = 1` writes a property and binds no local,
        // while `$rows['a']['b'] = 1` still creates `$rows` -- so the base is followed, and any
        // depth of indexing settles the same amount: that it is an array.
        ExprData::Index { base, .. } => Some(Bound::Element(bound_by(hir, *base)?.name())),
        _ => None,
    }
}

/// The names a call may bind through its arguments, with where each is written.
///
/// Every bare variable passed to a call: `preg_match($p, $s, $matches)` fills `$matches` through a
/// by-reference parameter and nothing at the call site says so -- which parameter is by reference
/// is the callee's business, and the callee may be anywhere. So any of them may be the one that
/// creates the name. `None`-shaped for an expression that is not a call.
///
/// One rule for every reader that asks what a position has bound: the early-read check and the
/// list of locals in scope used to write it out separately, and only one of them had it at all --
/// so completion never offered the `$matches` the line above had just filled.
pub(crate) fn argument_bindings<'h>(
    hir: &'h FileHir,
    call: ExprId,
    out: &mut impl FnMut(&'h Name, phrost_model::TextRange),
) {
    let args = match hir.expr(call) {
        ExprData::MethodCall { args, .. }
        | ExprData::StaticCall { args, .. }
        | ExprData::FunctionCall { args, .. }
        | ExprData::IndirectCall { args, .. }
        | ExprData::New { args, .. } => *args,
        _ => return,
    };
    for &argument in hir.expr_list(args) {
        let value = match hir.expr(argument) {
            ExprData::Argument { value, .. } => *value,
            _ => argument,
        };
        if let Some(bound) = bound_by(hir, value) {
            out(bound.name(), hir.expr(value).range());
        }
    }
}

/// Every name a destructuring target binds, in written order: `[$a, [$b], 'k' => $c] = $row`.
///
/// The enumerating half of [`destructured_slot`], for a reader that asks which names exist rather
/// than where one of them came from -- what is in scope after the statement. Without it a name bound
/// only by destructuring was in no list of locals, so completion offered every variable but the
/// one the line above had just written.
///
/// Whatever the key, unlike the slot: `[$this->key() => $v] = $row` still binds `$v`, it is only its
/// value that this layer cannot follow. An element target -- `[$parts[2]] = $pair` -- creates
/// `$parts` the way any write through an index does.
pub(crate) fn destructured_names<'h>(
    hir: &'h FileHir,
    target: ExprId,
    out: &mut impl FnMut(&'h Name),
) {
    let ExprData::ArrayLiteral { elements, .. } = hir.expr(target) else {
        return;
    };
    for &element in hir.expr_list(*elements) {
        let ExprData::ArrayElement { value, .. } = hir.expr(element) else {
            continue;
        };
        match bound_by(hir, *value) {
            Some(bound) => out(bound.name()),
            None => destructured_names(hir, *value, out),
        }
    }
}

/// The slot a destructuring target binds `name` to, outermost key first.
///
/// `[$first, $second] = $row` binds `$second` to `$row[1]`, and `[[$a], $b] = $rows` binds `$a` to
/// `$rows[0][0]` -- so the answer is a path of keys rather than one. Lowering writes the index on
/// every element of a target, holes counted, which is what makes this a read rather than a count;
/// see `destructuring_elements`.
///
/// `None` for a target that binds the name nowhere, and for one that binds it to a slot with no name
/// -- `[$this->key() => $v]` names a slot this layer cannot identify, and answering with the wrong
/// one is worse than answering nothing.
pub(crate) fn destructured_slot(hir: &FileHir, target: ExprId, name: &str) -> Option<Vec<Name>> {
    let ExprData::ArrayLiteral { elements, .. } = hir.expr(target) else {
        return None;
    };
    for &element in hir.expr_list(*elements) {
        let ExprData::ArrayElement { kind, value, .. } = hir.expr(element) else {
            continue;
        };
        let ArrayElementKind::LiteralKey { name: key, .. } = kind else {
            continue;
        };
        // The variable itself, and only as a *whole*: `[$parts[2], $parts[3]] = $pair` writes two
        // slots of `$parts` and says nothing about `$parts`, so reading it as that name's value
        // retyped the array as one of its own elements -- `implode(' ', $parts)` reported on a line
        // that works.
        if matches!(bound_by(hir, *value), Some(Bound::Whole(bound)) if bound.as_str() == name) {
            return Some(vec![key.clone()]);
        }
        if let Some(mut inner) = destructured_slot(hir, *value, name) {
            inner.insert(0, key.clone());
            return Some(inner);
        }
    }
    None
}
