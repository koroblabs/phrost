//! Array entries whose key is written out, and where their value sits.
//!
//! `['#type' => 'textfield']`. The key names what the value *means*, and in a framework's render and
//! configuration arrays that is the only thing in the source that does: there is no receiver to
//! resolve, no method to look up and no argument list -- just a pair. Everything else that resolves
//! a string in this project hangs off a call, so none of it can reach here.
//!
//! Two shapes of the same question, for the reason [`call_sites`](crate::call_sites) and
//! [`call_site_at`](crate::call_site_at) are two: a whole-file sweep builds one record per entry in
//! the file, and a cursor needs exactly one. On the path a keystroke takes that is the difference
//! between one allocation and a few thousand -- a real form-building method is nothing but these.
//!
//! Apart from [`element_written`](crate::element_written), which asks what went into one slot of a
//! *named container* and answers with a value path. This asks the plainer thing: what is written
//! here, and where.

use phrost_hir::{
    ExprId, FileHir,
    body::{ArrayElementKind, ExprData, LiteralKind},
};
use phrost_model::{Name, TextRange, TextSize};

use crate::scope::Scope;

/// One entry of an array literal, written with a key a literal spells out.
///
/// Usually the value beside the key. When that value is itself a flat list of bare strings --
/// `'library' => ['a/b', 'c/d']` -- the key governs every item in it, so one source pair produces
/// one `KeyedElement` per item, `key`/`key_range` repeated and `value` narrowed to that item alone.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyedElement {
    /// The key without its quotes, as the source wrote it. Compared exactly by every reader, because
    /// PHP compares an array key exactly.
    pub key: Name,
    /// Where the key itself is written, which is what a report about the *key* underlines.
    pub key_range: TextRange,
    /// Where the value is written, delimiters included when it is a literal -- the range an edit
    /// would replace and a report would underline.
    pub value: TextRange,
}

/// Every keyed entry in the file, in the order the walk reaches them.
///
/// Nested ones included, and no attempt is made to say which array an entry belongs to: what an
/// entry means is settled by its own key, and a reader that wanted the enclosing array would be
/// asking a different question.
pub fn keyed_elements(hir: &FileHir) -> Vec<KeyedElement> {
    let mut found = Vec::new();

    Scope::visit_every_expression(hir, &mut |hir, expr| {
        elements_of(hir, expr, &mut |element| found.push(element));
    });

    found
}

/// The keyed entry whose *value* `offset` sits in.
///
/// The value and not the whole entry: a cursor on the key is writing the key, and answering with
/// what the key means would offer element ids where property names belong.
///
/// Innermost, because `['#attached' => ['library' => ['a/b']]]` is three entries deep and only the
/// nearest one says what the string being written is.
pub fn keyed_element_at(hir: &FileHir, offset: TextSize) -> Option<KeyedElement> {
    let mut best: Option<KeyedElement> = None;

    Scope::visit_every_expression(hir, &mut |hir, expr| {
        // Built before the comparison, so an entry whose key this layer cannot spell is skipped
        // rather than taken and then dropped -- which would lose the outer entry that does answer.
        elements_of(hir, expr, &mut |element| {
            if !element.value.contains_inclusive(offset) {
                return;
            }
            if best
                .as_ref()
                .is_none_or(|held| element.value.len() < held.value.len())
            {
                best = Some(element);
            }
        });
    });

    best
}

/// The keyed entries of the innermost array literal `offset` sits inside.
///
/// The neighbours question, where [`keyed_element_at`] asks the entry one. A cursor writing a *key*
/// is inside no entry's value and so has no entry of its own -- what it has is siblings, and in a
/// framework's render array one of those siblings says what the whole array is: `'#type' =>
/// 'textfield'` beside the key being typed is the only statement anywhere that `#maxlength` is a
/// thing this array accepts.
///
/// Innermost for [`keyed_element_at`]'s reason: `['#type' => 'details', 'child' => ['#type' =>
/// 'textfield', |]]` is being written against the textfield, and the outer array's own `#type` is
/// about a different element.
///
/// Only the entries whose key is a literal, and the entry the cursor is *in* is among them -- a
/// half-written `'#ma'` is a keyed entry like any other and its own name is not something a caller
/// wants offered back. Filtering it out is the caller's, because only the caller knows which range
/// the cursor is replacing.
pub fn keyed_siblings_at(hir: &FileHir, offset: TextSize) -> Vec<KeyedElement> {
    let mut best: Option<(TextRange, Vec<KeyedElement>)> = None;

    Scope::visit_every_expression(hir, &mut |hir, expr| {
        let ExprData::ArrayLiteral { elements, range } = hir.expr(expr) else {
            return;
        };
        if !range.contains_inclusive(offset) {
            return;
        }
        if best
            .as_ref()
            .is_some_and(|(held, _)| held.len() <= range.len())
        {
            return;
        }

        let entries = hir
            .expr_list(*elements)
            .iter()
            .filter_map(|&item| match hir.expr(item) {
                ExprData::ArrayElement {
                    kind: ArrayElementKind::LiteralKey { name, range },
                    value,
                    ..
                } => Some(KeyedElement {
                    key: name.clone(),
                    key_range: *range,
                    value: hir.expr(*value).range(),
                }),
                _ => None,
            })
            .collect();
        best = Some((*range, entries));
    });

    best.map(|(_, entries)| entries).unwrap_or_default()
}

/// Every keyed entry an expression is, when it is one.
///
/// Usually zero or one, like the record it reports: a spread names no key, and a computed one
/// names no key this layer can spell. Two or more only for one shape, and deliberately --
/// `'library' => ['a/b', 'c/d']` is a key whose value is a flat list of bare strings, and the key
/// names *each* one rather than the list. `'#attached' => ['library' => [...], ...]` is the same
/// node shape with none of its own elements bare, so it falls through to the ordinary single
/// entry unchanged: what decides the split is not the key's name but whether the list holds
/// anything a caller could mistake for the array itself.
///
/// Reusing the one walk `keyed_elements` and `keyed_element_at` already run, rather than a second
/// pass over the list once a key matches -- the split is cheap exactly because it never costs an
/// expression a walk it would not have needed anyway.
fn elements_of(hir: &FileHir, expr: ExprId, push: &mut dyn FnMut(KeyedElement)) {
    let ExprData::ArrayElement {
        kind: ArrayElementKind::LiteralKey { name, range },
        value,
        ..
    } = hir.expr(expr)
    else {
        return;
    };

    if let ExprData::ArrayLiteral { elements, .. } = hir.expr(*value) {
        let mut any = false;
        for &item in hir.expr_list(*elements) {
            if let ExprData::Literal {
                kind: LiteralKind::String,
                range: item_range,
            } = hir.expr(item)
            {
                any = true;
                push(KeyedElement {
                    key: name.clone(),
                    key_range: *range,
                    value: *item_range,
                });
            }
        }
        if any {
            return;
        }
    }

    push(KeyedElement {
        key: name.clone(),
        key_range: *range,
        value: hir.expr(*value).range(),
    });
}
