//! A member reached on a value declared `object`, which names no class.
//!
//! `function format(object $supplier) { return $supplier->getAddress('city'); }` is a call
//! nothing can check: `object` is every class at once, so whether `getAddress()` exists depends on
//! what a caller passes. PHPStan reports it from level 2 as `method.notFound` on `object`;
//! `undefined-method` stays quiet, because the member may well be there at runtime and that check
//! speaks only where the index can prove it is not.
//!
//! Asked for rather than always on, for the reason that silence is the default: a project that
//! types its parameters `object` on purpose does it everywhere -- one real project has 108 such
//! accesses, `symfony/symfony` 1 604 -- and a project that wants the PHPStan reading switches it on.
//! A property an `object{...}` shape names is answered, not reached on `object`, and is not this.

use phrost_view::{IndexView, MemberAccessOutcome, MemberKind};

use super::{Check, Diagnostic, Severity};

pub(crate) const MEMBER_ON_OBJECT: &str = "member-on-object";

pub(super) struct MemberOnObject;

impl Check for MemberOnObject {
    fn asked_for(&self) -> Option<&'static str> {
        Some(MEMBER_ON_OBJECT)
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for access in view.member_accesses().iter() {
            if access.outcome != MemberAccessOutcome::AnyObject {
                continue;
            }
            // The same two guards `undefined-member` reads: `isset($x->y)` asks whether the member
            // is there, and `method_exists($x, 'y')` above the call already checked.
            if access.asked_if_set
                || view.member_existence_checked(access.range.start(), access.name.as_str())
            {
                continue;
            }
            let name = access.name.as_str();
            let member = match access.kind {
                MemberKind::Method => format!("{name}()"),
                MemberKind::Property => format!("${name}"),
                MemberKind::ClassConstant | MemberKind::EnumCase => name.to_owned(),
            };
            out.push(Diagnostic {
                range: access.range,
                // A warning: the member may exist on what arrives, and nothing here says it does
                // not -- only that nothing declared says it does.
                severity: Severity::Warning,
                code: MEMBER_ON_OBJECT,
                message: format!("Nothing declares {member} on object, which names no class"),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
