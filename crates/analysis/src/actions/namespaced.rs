//! Writing the namespace the autoloader will look for a file under.
//!
//! The repair for [`psr4-mismatch`](crate::diagnostics), and the whole of it is one edit: the
//! declaration says one thing and the directory says another, and the directory is not this
//! action's to move.
//!
//! Preferred, unlike every other refactor here. A class the autoloader cannot find is broken rather
//! than untidy, there is exactly one namespace it could declare, and nothing about the choice is a
//! judgement -- which is the whole of what preferred means.
//!
//! About the *file* rather than about a place in it, which is what
//! [`ActionSubject::WholeFile`](super::ActionSubject) means here: the squiggle is on the namespace
//! line and a person clicks it there, so a gate keyed to the class's name would miss exactly the
//! position the offer exists for. What keeps it cheap is that the question itself is -- one walk of
//! the file's classes and one comparison against the autoload map.
//!
//! Only the namespace. A file whose *name* does not match its class is the same kind of defect and
//! needs the file renamed, which is a workspace edit of a different shape and a decision about a
//! path a person may have git history for.

use phrost_view::Psr4Mismatch;

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};
use crate::diagnostics::psr4::PSR4_MISMATCH;

pub(super) struct FixNamespace;

impl Offer for FixNamespace {
    fn kind(&self) -> ActionKind {
        ActionKind::QuickFix
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::WholeFile
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let Some(Psr4Mismatch::Namespace {
            range,
            declared: Some(_),
            required,
        }) = sites.view().psr4_mismatch()
        else {
            return;
        };
        out.push(CodeAction {
            title: format!("Declare `{required}`"),
            edits: vec![TextEdit::replace(range, required)],
            preferred: true,
            kind: ActionKind::QuickFix,
            fixes: Some(PSR4_MISMATCH),
        });
    }
}
