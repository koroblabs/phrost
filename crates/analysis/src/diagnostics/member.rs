//! A member the class does not have.
//!
//! The narrowest useful claim in the language: `$repo->fnid()` is a fatal error at runtime, and a
//! typo in a method name is the single most common way a PHP file is broken.
//!
//! Only when the index can account for the whole class. A receiver with no type, a hierarchy with an
//! unresolved parent, or a class that answers unknown names through `__call` all mean the member may
//! well exist, and reporting those is what makes a checker something people switch off.

use phrost_view::{IndexView, MemberAccessOutcome, MemberKind};

use super::{Check, Diagnostic, Severity};

pub(crate) const UNDEFINED_METHOD: &str = "undefined-method";
pub(crate) const UNDEFINED_PROPERTY: &str = "undefined-property";
pub(crate) const UNDEFINED_CONSTANT: &str = "undefined-constant";

pub(super) struct UndefinedMember;

impl Check for UndefinedMember {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for access in view.member_accesses().iter() {
            let MemberAccessOutcome::MissingMember { class } = &access.outcome else {
                continue;
            };
            // `isset($job->retryUntil)` is how a file asks whether the property is there, and PHP
            // answers `false` rather than warning -- so the report would land on the question.
            if access.asked_if_set {
                continue;
            }

            // A member the author already checked for at runtime is not a member they forgot.
            // `method_exists` is PHP's only way of writing "this may be on a subclass", so speaking
            // here means arguing with a guard that was written on purpose -- and the report lands on
            // the one line that proves the author knew.
            //
            // Asked last, because it walks the scope chain and only the handful of accesses that
            // would otherwise be reported ever reach it.
            if view.member_existence_checked(access.range.start(), access.name.as_str()) {
                continue;
            }
            // And the one PHP spells with a single string: `defined('Normalizer::NFKC_CF')`, which
            // is how a file says a *constant* may be absent. Both halves of the name are in that
            // string, so the class is compared too -- see `tests_class_constant`.
            if matches!(
                access.kind,
                MemberKind::ClassConstant | MemberKind::EnumCase
            ) && view.class_constant_existence_checked(class.as_str(), access.name.as_str())
            {
                continue;
            }

            out.push(Diagnostic {
                range: access.range,
                severity: Severity::Error,
                code: code_for(access.kind),
                message: message(class.as_str(), access.kind, access.name.as_str()),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

fn code_for(kind: MemberKind) -> &'static str {
    match kind {
        MemberKind::Method => UNDEFINED_METHOD,
        MemberKind::Property => UNDEFINED_PROPERTY,
        MemberKind::ClassConstant | MemberKind::EnumCase => UNDEFINED_CONSTANT,
    }
}

fn message(class_fqn: &str, kind: MemberKind, name: &str) -> String {
    // An anonymous class is named by where its keyword sits, which is meaningless to a reader --
    // PHP's own errors say `class@anonymous` and so does this.
    let class_fqn = phrost_view::display_class_name(class_fqn);
    match kind {
        MemberKind::Method => format!("Undefined method {class_fqn}::{name}()"),
        MemberKind::Property => format!("Undefined property {class_fqn}::${name}"),
        MemberKind::ClassConstant | MemberKind::EnumCase => {
            format!("Undefined constant {class_fqn}::{name}")
        }
    }
}
