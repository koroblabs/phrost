//! A `self`, `static` or `parent` written where a class goes.
//!
//! Its own lookup because the keyword is not a name reference and not a member site, so neither of
//! the two searches a cursor already runs can see it -- and goto and hover on the `self` of
//! `new self()` had nothing at all to find.

use phrost_hir::{FileHir, body::ClassTarget};
use phrost_model::{RelativeTy, TextRange, TextSize};

use crate::scope::Scope;

/// The relative class keyword `offset` sits on, if it sits on one.
pub fn relative_class_at(hir: &FileHir, offset: TextSize) -> Option<(RelativeTy, TextRange)> {
    let mut best: Option<(RelativeTy, TextRange)> = None;

    Scope::visit_every_expression(hir, &mut |hir, expr| {
        let Some(ClassTarget::Relative { relative, range }) = class_target_of(hir, expr) else {
            return;
        };
        if !range.contains_inclusive(offset) {
            return;
        }
        // Narrowest wins, the same rule every other position lookup uses.
        if best.is_none_or(|(_, current)| range.len() < current.len()) {
            best = Some((relative, range));
        }
    });

    best
}

/// The class an expression names, for the shapes that name one.
fn class_target_of(hir: &FileHir, expr: phrost_hir::ExprId) -> Option<ClassTarget> {
    use phrost_hir::body::ExprData;

    match hir.expr(expr) {
        ExprData::New { class, .. }
        | ExprData::StaticCall { class, .. }
        | ExprData::StaticPropertyAccess { class, .. }
        | ExprData::ClassConstAccess { class, .. }
        | ExprData::Instanceof { class, .. } => Some(*class),
        // `self::find(...)` writes the keyword where any other `::` does.
        ExprData::CallableMember {
            receiver: phrost_hir::body::CallableReceiver::Class(class),
            ..
        } => Some(*class),
        _ => None,
    }
}
