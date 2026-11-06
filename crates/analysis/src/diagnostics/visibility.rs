//! A member reached from where PHP will not allow it.
//!
//! `Cannot access private property` is a fatal error, and the rule behind it is about *classes*
//! rather than objects -- inside `Repo`, another `Repo` instance's private members are reachable.
//! That is the part easiest to get backwards, and it is settled one layer down.
//!
//! Silent where PHP has another route: a class with `__get` or `__call` answers a name it cannot
//! reach directly, so an inaccessible member there is a call to the magic method. Without that
//! exemption this would report every framework class with a magic accessor.

use phrost_model::Visibility;
use phrost_view::{IndexView, MemberAccessOutcome, MemberKind};

use super::{Check, Diagnostic, Severity};

pub(crate) const INACCESSIBLE_MEMBER: &str = "inaccessible-member";

pub(super) struct InaccessibleMember;

impl Check for InaccessibleMember {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for access in view.member_accesses().iter() {
            let MemberAccessOutcome::Resolved { class } = &access.outcome else {
                continue;
            };
            // A double configures a protected method by name as readily as a public one, and
            // nothing is called from the test that names it. And `isset()` on a private property
            // from outside is `false`, not an error.
            if access.named || access.asked_if_set {
                continue;
            }
            let Some(declared) = view.inaccessible_member(
                access.range.start(),
                class.as_str(),
                access.kind,
                access.name.as_str(),
            ) else {
                continue;
            };

            out.push(Diagnostic {
                range: access.range,
                severity: Severity::Error,
                code: INACCESSIBLE_MEMBER,
                message: format!(
                    "Cannot access {} {} {}::{}",
                    word_for(declared),
                    noun_for(access.kind),
                    class,
                    written(access.kind, access.name.as_str())
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

fn word_for(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Private => "private",
        Visibility::Protected => "protected",
        // Never reached: a public member is visible from everywhere, so it never gets here.
        Visibility::Public => "public",
    }
}

fn noun_for(kind: MemberKind) -> &'static str {
    match kind {
        MemberKind::Method => "method",
        MemberKind::Property => "property",
        MemberKind::ClassConstant => "constant",
        MemberKind::EnumCase => "case",
    }
}

fn written(kind: MemberKind, name: &str) -> String {
    match kind {
        MemberKind::Property => format!("${name}"),
        MemberKind::Method => format!("{name}()"),
        MemberKind::ClassConstant | MemberKind::EnumCase => name.to_owned(),
    }
}
