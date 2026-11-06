//! Correcting a call's label to the parameter it is one edit from -- the fix for
//! `unknown-named-parameter`. `find(idd: 5)` becomes `find(id: 5)`.

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit, concerns};
use crate::diagnostics::named_arguments::UNKNOWN_NAMED_PARAMETER;

pub(super) struct CorrectNamedLabel;

impl Offer for CorrectNamedLabel {
    fn kind(&self) -> ActionKind {
        ActionKind::QuickFix
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::Expression
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let view = sites.view();
        let at = sites.at();
        // Cheaply first: a label is a word with a single `:` after it, and an action request lands
        // on every cursor movement in some editors.
        let Some(text) = view.current_text() else {
            return;
        };
        let start = usize::from(at.start()).min(text.len());
        let end = usize::from(at.end()).clamp(start, text.len());
        let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
        if !text[line_start..line_end].contains(':') {
            return;
        }
        for misnamed in view.misnamed_arguments() {
            let Some(nearest) = &misnamed.nearest else {
                continue;
            };
            if !concerns(misnamed.range, at) {
                continue;
            }
            out.push(CodeAction {
                title: format!("Change ${} to ${nearest}", misnamed.name),
                edits: vec![TextEdit::replace(
                    misnamed.range,
                    nearest.as_str().to_owned(),
                )],
                preferred: true,
                kind: ActionKind::QuickFix,
                fixes: Some(UNKNOWN_NAMED_PARAMETER),
            });
        }
    }
}
