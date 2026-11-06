//! Making a call that may reach null handle it.
//!
//! `$node->getTitle()` where `$node` is a `?Node` is the commonest real bug in typed PHP, and
//! `$node?->getTitle()` is what a person writes to fix it -- one character, at a place the index can
//! point at exactly.
//!
//! It is a fix and not a silencer. `?->` changes the expression's value to null on the runs where the
//! receiver is null, so whatever consumes it is now the thing that may see a null -- and if that
//! consumer cannot take one, the next check says so. Nothing is being suppressed; the null is being
//! moved to where it can be answered.

use phrost_view::{MemberKind, SentinelReach};

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};
use crate::diagnostics::nullable::POSSIBLY_NULL_RECEIVER;

pub(super) struct NullsafeCall;

impl Offer for NullsafeCall {
    fn kind(&self) -> ActionKind {
        ActionKind::QuickFix
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::MemberAccess
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let view = sites.view();
        for access in sites.accesses() {
            // A value that is null on *every* run has nothing to reach for: `?->` would make the
            // whole expression unconditionally null, which is not a fix but a different bug.
            if access.null_reach != SentinelReach::Possible {
                continue;
            }
            // A constant or an enum case is reached through the class, not through a value, so there
            // is no `->` to make safe -- and `?::` is not PHP.
            if matches!(
                access.kind,
                MemberKind::ClassConstant | MemberKind::EnumCase
            ) {
                continue;
            }
            let Some(operator) = view.operator_of(access) else {
                continue;
            };
            // Written `?->` already, or reached statically. Neither has anything to change.
            let text = match view.text_in(operator) {
                Some("->") => "?->",
                _ => continue,
            };

            out.push(CodeAction {
                title: match access.kind {
                    MemberKind::Method => format!("Call {}() only when not null", access.name),
                    _ => format!("Read ${} only when not null", access.name),
                },
                edits: vec![TextEdit::replace(operator, text)],
                // The only fix on offer for this, and the one a person means.
                preferred: true,
                kind: ActionKind::QuickFix,
                // `SentinelReach::Possible` is what the warning-level check reports; the certain case
                // has nothing for `?->` to help with and was skipped above.
                fixes: Some(POSSIBLY_NULL_RECEIVER),
            });
        }
    }
}
