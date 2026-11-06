//! A name a class declares twice.
//!
//! `Cannot redeclare C::m()` is a fatal at class load, so the file does not run at all. See
//! [`redeclared_members`](phrost_view::IndexView::redeclared_members) for which names collide --
//! three namespaces, and methods compare folded while properties compare exactly.

use phrost_view::{IndexView, MemberKind};

use super::{Check, Diagnostic, Severity};

pub(crate) const REDECLARED_MEMBER: &str = "redeclared-member";

pub(super) struct RedeclaredMember;

impl Check for RedeclaredMember {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.redeclared_members() {
            out.push(Diagnostic {
                // The second declaration, which is the one a reader would delete.
                range: found.range,
                severity: Severity::Error,
                code: REDECLARED_MEMBER,
                message: format!(
                    "{} already declares {}{}",
                    found.class,
                    match found.kind {
                        MemberKind::Property => "$",
                        _ => "",
                    },
                    found.member
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
