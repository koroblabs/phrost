//! Renaming a docblock tag no tool reads to the one it plainly means -- the fix for
//! `doc-misspelt-tag`. One word, and the type written beside it comes back into every reader.

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit, concerns};
use crate::diagnostics::misspelt_tags::{DOC_MISSPELT_TAG, MisspeltTag};

pub(super) struct RenameMisspeltTag;

impl Offer for RenameMisspeltTag {
    fn kind(&self) -> ActionKind {
        ActionKind::QuickFix
    }

    /// Asked of every request, since a docblock is none of the other subjects -- so the offer
    /// gates itself on the line under the cursor holding a tag at all, which nearly none do.
    fn subject(&self) -> ActionSubject {
        ActionSubject::Expression
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let view = sites.view();
        let at = sites.at();
        let Some(text) = view.current_text() else {
            return;
        };
        // The lines the range touches, whole: a cursor on a tag is on its line, and the whole-file
        // request `source.fixAll` makes covers every line there is.
        let start = usize::from(at.start()).min(text.len());
        let end = usize::from(at.end()).clamp(start, text.len());
        let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
        if !text[line_start..line_end].contains('@') {
            return;
        }
        for tag in MisspeltTag::in_file(view) {
            if !concerns(tag.range, at) {
                continue;
            }
            // The weaker tag out first in the list, which is also first in the file or not -- the
            // two ranges never touch, since one is a tag's name and the other whole lines of
            // another tag.
            let mut edits = vec![TextEdit::replace(tag.range, tag.meant)];
            let title = match tag.replaces {
                None => format!("Replace @{} with @{}", tag.written, tag.meant),
                Some(block) => {
                    edits.push(TextEdit::replace(block, ""));
                    format!(
                        "Replace @{} with @{}, dropping the @{} that says less",
                        tag.written, tag.meant, tag.meant
                    )
                }
            };
            edits.sort_by_key(|edit| edit.range.start());
            out.push(CodeAction {
                title,
                edits,
                preferred: true,
                kind: ActionKind::QuickFix,
                fixes: Some(DOC_MISSPELT_TAG),
            });
        }
    }

    /// Changes what a comment says and nothing the code does, and says what the author meant.
    fn unattended(&self) -> bool {
        true
    }
}
