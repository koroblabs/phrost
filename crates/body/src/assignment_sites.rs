//! Where the file assigns something, and what it assigns.
//!
//! A whole-file projection, for anything that has to line a value up against the type its target
//! declares. The target is described as written -- a variable, a member reached through a path, a
//! static member on a class target -- because deciding what it resolves to needs a catalog, which
//! nothing here has. Same division as `calls`.
//!
//! Named apart from [`crate::assignments`] in `variables` on purpose. That one enumerates plain
//! variables only and exists to *measure* how much of a project's inference works; widening it would
//! change what the coverage sweep reports, which is a different question from the one asked here.

use phrost_hir::{
    ExprId, FileHir, TypeRefId,
    body::{ClassTarget, ExprData},
};
use phrost_model::{Name, TextRange};

use crate::{ValuePath, path::value_path_of, scope::Scope};

/// One `=` written in the file.
///
/// Only `=`: lowering already refuses a compound assignment, because `+=` produces a value the
/// operator decides rather than the right-hand side, and calling it an assignment would hand the
/// target the wrong type.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AssignmentSite {
    pub target: AssignmentTarget,
    /// The target as written, which is what a report underlines.
    pub target_range: TextRange,
    /// The value's **own** path, with no annotation applied.
    ///
    /// Deliberately not what `path_of` gives for the same expression. That substitutes a
    /// `/** @var T */` for the value, because for *inference* the annotation is the better answer --
    /// it is written precisely when the value does not say. For checking, the two have to stay
    /// apart: the annotation is what is expected and the value is what arrived, and collapsing them
    /// makes every annotated assignment agree with itself.
    pub value: Option<ValuePath>,
    /// The value's expression, for a check that needs the source as *written* rather than what it
    /// resolves to -- whether a literal is a number, which no path can say.
    pub value_expr: ExprId,
    pub value_range: TextRange,
    /// A `/** @var T */` written above the statement, when there is one.
    pub annotation: Option<TypeRefId>,
}

/// What is being assigned to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AssignmentTarget {
    /// `$x = ...`.
    Variable(Name),
    /// `$obj->x = ...`, reached through a value.
    Property { receiver: ValuePath, name: Name },
    /// `C::$x = ...`.
    StaticProperty { class: ClassTarget, name: Name },
}

/// Every assignment written in the file, in the order the walk reaches them.
pub fn assignment_sites(hir: &FileHir) -> Vec<AssignmentSite> {
    let mut sites = Vec::new();

    // Closures included, for the reason the call walk includes them: an assignment inside one is an
    // assignment, and a check blind to them would be quietly partial on modern code.
    Scope::visit_every_expression(hir, &mut |hir, expr| {
        if let Some(site) = site_of(hir, expr) {
            sites.push(site);
        }
    });

    sites
}

/// Every property a `clone($object, [...])` writes on its copy, as the assignment it is.
///
/// Apart from [`assignment_sites`] on purpose: the write is to the copy, and the readers of that
/// list learn facts from it about the object written through -- which properties a class assigns,
/// what a slot holds after the line. A type check is the one reader both are for.
pub fn cloned_property_writes(hir: &FileHir) -> Vec<AssignmentSite> {
    let mut sites = Vec::new();
    Scope::visit_every_expression(hir, &mut |hir, expr| {
        let Some(clone) = crate::clone_with::clone_with(hir, expr) else {
            return;
        };
        let Some(receiver) = value_path_of(hir, clone.object) else {
            return;
        };
        for entry in clone.entries {
            sites.push(AssignmentSite {
                target: AssignmentTarget::Property {
                    receiver: receiver.clone(),
                    name: entry.name,
                },
                target_range: entry.key_range,
                value: value_path_of(hir, entry.value),
                value_expr: entry.value,
                value_range: hir.expr(entry.value).range(),
                annotation: None,
            });
        }
    });
    sites
}

/// The site one expression is, when it is an assignment whose target this layer can name.
fn site_of(hir: &FileHir, expr: ExprId) -> Option<AssignmentSite> {
    let ExprData::Assign {
        target,
        value,
        doc_type_ref,
        ..
    } = hir.expr(expr)
    else {
        return None;
    };

    let target_range = hir.expr(*target).range();
    let target = match hir.expr(*target) {
        ExprData::Variable { name, .. } => AssignmentTarget::Variable(name.clone()),
        // A computed name -- `$obj->$field = ...` -- names no member, and guessing one would be a
        // claim about a property nobody wrote.
        ExprData::PropertyAccess {
            receiver,
            name: Some(name),
            ..
        } => AssignmentTarget::Property {
            receiver: value_path_of(hir, *receiver)?,
            name: name.clone(),
        },
        ExprData::StaticPropertyAccess {
            class,
            name: Some(name),
            ..
        } => AssignmentTarget::StaticProperty {
            class: *class,
            name: name.clone(),
        },
        // A list destructuring, an array append, a computed member: each assigns something this
        // layer cannot name, and naming it wrongly is worse than not reporting it.
        _ => return None,
    };

    Some(AssignmentSite {
        target,
        target_range,
        value: value_path_of(hir, *value),
        value_expr: *value,
        value_range: hir.expr(*value).range(),
        annotation: *doc_type_ref,
    })
}
