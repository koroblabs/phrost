//! Turning a constructor parameter and the property it fills into one declaration.
//!
//! `__construct(private string $title)` is how modern PHP writes a constructor -- 1 508 promoted
//! properties in 431 files on one real checkout -- and the older spelling is three places that have
//! to agree: the property, the parameter, and the line between them. Promoting is deleting two of
//! the three.
//!
//! Every judgement about *whether* the three are equivalent belongs to
//! [`promotable_at`](phrost_view::IndexView::promotable_at), which refuses seven shapes and says why
//! for each. This lays out the edits.
//!
//! A refactor. The older spelling is not wrong, it is older -- and a class whose properties are
//! documented one by one is a class somebody wrote that way on purpose.

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit, concerns};

pub(super) struct PromoteParameter;

impl Offer for PromoteParameter {
    fn kind(&self) -> ActionKind {
        ActionKind::Refactor
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::Declaration
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let (view, at) = (sites.view(), sites.at());
        // The parameter under the cursor, which is where the offer belongs: it is the half that
        // survives, and the half a person is looking at when they think of promoting.
        let Some(parameter) = sites
            .declarations()
            .flat_map(|declaration| declaration.parameters.iter())
            .find(|parameter| concerns(parameter.name_range, at))
        else {
            return;
        };
        let Some(found) = view.promotable_at(parameter.name_range.start()) else {
            return;
        };

        out.push(CodeAction {
            title: format!("Promote ${} into the constructor", found.name),
            // Three, and none of them overlaps another: the property is above the signature, the
            // modifiers go into it, and the assignment is below it.
            edits: vec![
                TextEdit::replace(found.property, String::new()),
                TextEdit::insert(found.at, found.modifiers),
                TextEdit::replace(found.assignment, String::new()),
            ],
            // Nothing is wrong, so nothing is preferred.
            preferred: false,
            kind: ActionKind::Refactor,
            fixes: None,
        });
    }
}
