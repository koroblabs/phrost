//! Lifting a selected expression into a variable.
//!
//! Two edits: the assignment above the statement, and the name in place of what was selected. What
//! may be lifted at all is [`extraction_at`](phrost_view::IndexView::extraction_at) and the
//! [`phrost_body`] finder under it, which refuse a short-circuit, a branch and a closure -- each
//! because hoisting would run the code when it did not before.
//!
//! Offered on a selection *and* on a bare cursor. A person selecting an expression has said which
//! one; a person with the caret in one has said it less precisely and gets the innermost thing
//! worth a name, which is the same rule every editor applies.

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};

pub(super) struct ExtractVariable;

impl Offer for ExtractVariable {
    fn kind(&self) -> ActionKind {
        ActionKind::Refactor
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::Expression
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let Some(found) = sites.view().extraction_at(sites.at()) else {
            return;
        };
        out.push(CodeAction {
            title: format!("Extract ${} ", found.name).trim_end().to_owned(),
            edits: vec![
                TextEdit::insert(
                    found.at,
                    format!(
                        "${} = {};\n{}",
                        found.name,
                        sites
                            .view()
                            .text_in(found.range)
                            .unwrap_or_default()
                            .to_owned(),
                        found.indent
                    ),
                ),
                TextEdit::replace(found.range, format!("${}", found.name)),
            ],
            // Nothing is wrong, and where to stop naming things is the author's.
            preferred: false,
            kind: ActionKind::Refactor,
            fixes: None,
        });
    }
}
