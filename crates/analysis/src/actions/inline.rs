//! Putting a variable's value back where its name is used.
//!
//! The other direction of [`extract`](super::extract), and the danger is the mirror image:
//! extracting makes an expression run earlier, and inlining makes it run as many times as the name
//! is read. Which reads may be replaced at all is
//! [`inlining_at`](phrost_view::IndexView::inlining_at), and it says why for each refusal.

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};

pub(super) struct InlineVariable;

impl Offer for InlineVariable {
    fn kind(&self) -> ActionKind {
        ActionKind::Refactor
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::Expression
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let view = sites.view();
        let Some(found) = view.inlining_at(sites.at().start()) else {
            return;
        };
        let Some(written) = view.text_in(found.value) else {
            return;
        };
        // Parenthesised where the value is not a primary: a name binds tighter than every
        // operator, and the expression replacing it may not. See `Inlining::needs_parentheses`.
        let value = match found.needs_parentheses {
            true => format!("({written})"),
            false => written.to_owned(),
        };

        let mut edits = vec![TextEdit::replace(found.statement, String::new())];
        edits.extend(
            found
                .reads
                .iter()
                .map(|read| TextEdit::replace(*read, value.clone())),
        );
        out.push(CodeAction {
            title: format!("Inline ${}", found.name),
            edits,
            // Naming a value is a decision somebody made, and un-making it is another.
            preferred: false,
            kind: ActionKind::Refactor,
            fixes: None,
        });
    }
}
