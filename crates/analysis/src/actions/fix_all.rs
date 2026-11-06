//! Every fix in the file that can be applied without anybody choosing, as one edit.
//!
//! The protocol's `source.fixAll`, which an editor binds to saving or to a menu entry. What goes in
//! is decided by the offers themselves: an offer says it is [`Offer::unattended`] when applying its
//! *preferred* action changes nothing a person would want to read first -- a `@param` line, an import
//! with one candidate. The nullsafe operator is the counter-example: it repairs a real finding and
//! changes what the code does, and a save must not do that to a file.
//!
//! Non-overlapping by construction. Two fixes whose edits touch -- two inserts at one offset, or a
//! replacement across an insert -- would leave the result to whoever applies them, so the second is
//! left for the next save rather than guessed at.

use phrost_model::{TextRange, TextSize};

use super::{
    ActionKind, ActionRequest, ActionSubject, CodeAction, OFFERS, Offer, SubjectSites, TextEdit,
};

pub(super) struct FixAll;

impl Offer for FixAll {
    fn kind(&self) -> ActionKind {
        ActionKind::FixAll
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::WholeFile
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let view = sites.view();
        let Some(text) = view.current_text() else {
            return;
        };
        let Ok(end) = TextSize::try_from(text.len()) else {
            return;
        };
        // The same offers, asked about the whole file: what they gate on a range they now gate on
        // everything, and what each marks preferred is what a keystroke would have taken at each
        // site in turn.
        let request = ActionRequest {
            view,
            at: TextRange::new(TextSize::from(0), end),
            only: &[ActionKind::QuickFix],
            asked: sites.asked(),
        };
        let everywhere = SubjectSites::of(&request);
        let mut fixes = Vec::new();
        for offer in OFFERS {
            if offer.kind() != ActionKind::QuickFix || !offer.unattended() {
                continue;
            }
            if !everywhere.holds(offer.subject()) {
                continue;
            }
            offer.run(&everywhere, &mut fixes);
        }

        let mut taken: Vec<TextRange> = Vec::new();
        let mut edits: Vec<TextEdit> = Vec::new();
        let mut applied = 0_usize;
        // Only fixes for what the project reports. A `@throws` line is a fine thing to offer anywhere
        // and a poor thing to write into every file a person saves, in a project that never asked
        // for the check: Laravel would have gained 2 321 boilerplate tag lines across 419 files.
        let asked = sites.asked();
        let reported = |fix: &CodeAction| fix.fixes.is_none_or(|code| asked.reports(code));
        for fix in fixes
            .into_iter()
            .filter(|fix| fix.preferred && reported(fix))
        {
            if fix
                .edits
                .iter()
                .any(|edit| taken.iter().any(|held| touches(*held, edit.range)))
            {
                continue;
            }
            taken.extend(fix.edits.iter().map(|edit| edit.range));
            edits.extend(fix.edits);
            applied += 1;
        }
        if edits.is_empty() {
            return;
        }
        edits.sort_by_key(|edit| (edit.range.start(), edit.range.end()));

        out.push(CodeAction {
            title: match applied {
                1 => "Fix the 1 problem phrost can fix".to_owned(),
                many => format!("Fix the {many} problems phrost can fix"),
            },
            edits,
            // Asked for by kind, never taken by a keystroke: a `source` action is something a
            // person or a save asks for.
            preferred: false,
            kind: ActionKind::FixAll,
            fixes: None,
        });
    }
}

/// Whether two edit ranges cannot be applied together without one deciding the other's place.
///
/// Inclusive at both ends, so two inserts at one offset count: their order would be the applier's.
fn touches(left: TextRange, right: TextRange) -> bool {
    left.start() <= right.end() && right.start() <= left.end()
}
