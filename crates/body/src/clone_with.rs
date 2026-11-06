//! PHP 8.5's `clone($object, ['x' => 2])`: a copy, and a write to each property the array names.
//!
//! Asked of PHP 8.5.10, every entry is the property write `$copy->x = 2` made from the scope the
//! call is written in -- an undeclared name creates a dynamic property, a value of the wrong type is
//! a `TypeError`, a private property is an `Error` -- and the copy is of the object's own class. So
//! each entry is read as two sites the rest of the checker already judges: a member named on the
//! object, and an assignment to it. Never as a write to the *object*: the original is untouched, and
//! a reader that took the entry for one would narrow a property nothing wrote.

use phrost_hir::{
    ExprId, FileHir,
    body::{ArrayElementKind, ExprData},
};
use phrost_model::{Name, TextRange, TextSize};

/// One `clone()` written with properties.
pub(crate) struct CloneWith {
    /// The object cloned, whose class the copy is.
    pub(crate) object: ExprId,
    pub(crate) entries: Vec<CloneEntry>,
}

/// One property the array names.
pub(crate) struct CloneEntry {
    pub(crate) name: Name,
    /// The name between its quotes, which is what goto and rename act on.
    pub(crate) name_range: TextRange,
    /// The key as written, quotes included, which is what a report underlines.
    pub(crate) key_range: TextRange,
    pub(crate) value: ExprId,
}

/// The call `expr` is, when it is a `clone()` with a literal array of properties.
///
/// Only keys written as literals that spell a property name: a computed key names nothing this
/// layer can read, and `0 => ...` names a property no class can declare.
pub(crate) fn clone_with(hir: &FileHir, expr: ExprId) -> Option<CloneWith> {
    let ExprData::FunctionCall { name, args, .. } = hir.expr(expr) else {
        return None;
    };
    if !hir
        .name_ref(*name)
        .path
        .as_str()
        .trim_start_matches('\\')
        .eq_ignore_ascii_case("clone")
    {
        return None;
    }
    let [object, properties] = hir.expr_list(*args) else {
        return None;
    };
    let ExprData::ArrayLiteral { elements, .. } = hir.expr(positional(hir, *properties)?) else {
        return None;
    };
    let entries = hir
        .expr_list(*elements)
        .iter()
        .filter_map(|&element| {
            let ExprData::ArrayElement {
                kind: ArrayElementKind::LiteralKey { name, range },
                value,
                ..
            } = hir.expr(element)
            else {
                return None;
            };
            if !spells_a_property(name.as_str()) {
                return None;
            }
            Some(CloneEntry {
                name: name.clone(),
                name_range: between_the_quotes(name, *range),
                key_range: *range,
                value: *value,
            })
        })
        .collect();
    Some(CloneWith {
        object: positional(hir, *object)?,
        entries,
    })
}

/// An argument's value, unless it is spread -- which hands over values nothing here can name.
fn positional(hir: &FileHir, argument: ExprId) -> Option<ExprId> {
    match hir.expr(argument) {
        ExprData::Argument {
            value,
            spread: false,
            ..
        } => Some(*value),
        ExprData::Argument { .. } => None,
        _ => Some(argument),
    }
}

fn spells_a_property(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic() || !first.is_ascii())
}

/// The name's own range inside a quoted key: one character in from each end when the key is the
/// name and its two quotes, and the whole key when an escape makes the two disagree.
fn between_the_quotes(name: &Name, key: TextRange) -> TextRange {
    let quoted = TextSize::try_from(name.len() + 2).ok();
    match quoted == Some(key.len()) {
        true => TextRange::new(
            key.start() + TextSize::from(1),
            key.end() - TextSize::from(1),
        ),
        false => key,
    }
}
