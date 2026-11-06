//! A member reached through a value no object can be.
//!
//! `$name->format()` where `$name` is a string. PHP raises `Call to a member function format() on
//! string` and stops, so unlike everything else about a receiver this is not an absence of knowledge
//! -- it is proof, and the one case where saying nothing costs a person a runtime fatal.
//!
//! Its own check rather than part of the undefined-member one, because the two answer different
//! questions: that one asks whether a class has a member, and this one asks whether there is a class
//! at all. A value that is a string has no member table to be missing from.

use phrost_view::{IndexView, MemberAccessOutcome, MemberKind};

use super::{Check, Diagnostic, Severity};

pub(crate) const NON_OBJECT_RECEIVER: &str = "non-object-receiver";

pub(super) struct NonObjectReceiver;

impl Check for NonObjectReceiver {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for access in view.member_accesses().iter() {
            let MemberAccessOutcome::NotAnObject { kind } = access.outcome else {
                continue;
            };
            out.push(Diagnostic {
                range: access.range,
                // An error: PHP stops, which is the difference between this and a nullable receiver
                // that may or may not arrive.
                severity: Severity::Error,
                code: NON_OBJECT_RECEIVER,
                message: format!(
                    "Cannot {} {} on {}",
                    match access.kind {
                        MemberKind::Method => "call method",
                        MemberKind::Property => "access property",
                        MemberKind::ClassConstant | MemberKind::EnumCase => "read constant",
                    },
                    match access.kind {
                        MemberKind::Method => format!("{}()", access.name),
                        _ => format!("${}", access.name),
                    },
                    kind.as_str()
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
