//! Offering `{@inheritdoc}` instead of a full docblock, for an override that has none.
//!
//! `document::WriteDocblock` always offers to write the whole thing from the signature, which is
//! correct but not the one a person overriding an interface method usually wants -- PHPDoc's own
//! shorthand for "this is documented above" is shorter, does not go stale the way a copy would,
//! and is exactly what the standard expects here. Offered beside `WriteDocblock` rather than
//! instead of it: nothing here can tell which reading a person actually wants, only that this one
//! is available, which is what makes it a second offer and not a replacement.
//!
//! Refused unless there genuinely is a declaration above this one to inherit from --
//! [`IndexView::overridden_member`] is, by its own doc, exactly what `{@inheritdoc}` needs, so
//! answering `None` means the tag would document nothing and this stays silent rather than write
//! a promise the file cannot keep.

use phrost_view::MemberKind;

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};
use crate::diagnostics::docblocks::DOC_MISSING_TAGS;

pub(super) struct InheritDocblock;

impl Offer for InheritDocblock {
    fn kind(&self) -> ActionKind {
        ActionKind::QuickFix
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::Declaration
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let (view, at) = (sites.view(), sites.at());
        let Some(found) = sites.documentable() else {
            return;
        };
        // Offering a second docblock would be offering to break the file -- the same guard
        // `WriteDocblock` runs, for the same reason.
        if found.documented {
            return;
        }
        let Some(owner) = view.enclosing_class(at.start()) else {
            return;
        };
        if view
            .overridden_member(owner.as_str(), MemberKind::Method, found.name.as_str())
            .is_none()
        {
            return;
        }

        let indent = &found.indent;
        out.push(CodeAction {
            title: "Document with {@inheritdoc}".to_owned(),
            edits: vec![TextEdit::insert(
                found.at,
                format!("/**\n{indent} * {{@inheritdoc}}\n{indent} */\n{indent}"),
            )],
            // `WriteDocblock` stands at the same place with a real alternative, so neither is the
            // one candidate a keystroke should pick without being asked.
            preferred: false,
            kind: ActionKind::QuickFix,
            fixes: Some(DOC_MISSING_TAGS),
        });
    }
}
