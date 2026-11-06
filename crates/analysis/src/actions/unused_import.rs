//! Deleting a `use` line nothing in the file names.
//!
//! Not a repair -- the file already runs correctly, the same way an unused local does -- so this
//! is offered rather than reported: see [`super::ActionKind::Refactor`]. What makes the edit safe
//! is proving the import is alone on its line before touching anything, because [`IndexView`]
//! hands back only the *path*'s range, not the statement's -- see the field doc on
//! [`phrost_view::UnusedImport::range`]. A grouped import (`use App\{Repo, Model};`) shares its
//! statement with another name the file may still use, and a flat list (`use App\Repo, App\Model;`)
//! does the same without the braces, so this only ever removes an import that turns out to be the
//! whole of its own `use` statement -- everything else is left alone rather than guessed at.

use phrost_model::{TextRange, TextSize};
use phrost_view::{ImportKind, IndexView, UnusedImport};

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};

pub(super) struct RemoveUnusedImport;

impl Offer for RemoveUnusedImport {
    fn kind(&self) -> ActionKind {
        ActionKind::Refactor
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::Import
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let view = sites.view();
        for unused in sites.imports() {
            let Some(edit) = removal_edit(view, unused) else {
                continue;
            };
            out.push(CodeAction {
                title: format!("Remove unused import {}", unused.path),
                edits: vec![edit],
                // Nothing is wrong with the file, so nothing here is the one a keystroke should
                // apply unasked -- see `ActionKind::Refactor`.
                preferred: false,
                kind: ActionKind::Refactor,
                fixes: None,
            });
        }
    }
}

/// The edit that removes `unused`'s whole `use` statement, when it can be proven to have none of
/// its own besides this one name.
///
/// Proven from the raw text either side of the path, because that is the only place the answer
/// is written down: the path's own range is never wider than the path, whether the statement
/// holds one name or several.
pub(super) fn removal_edit(view: &IndexView<'_>, unused: &UnusedImport) -> Option<TextEdit> {
    // A grouped entry's range is the tail alone -- `Repo` between the braces of
    // `use App\{Repo, Model}` -- so it never spans the same text as the fully qualified path. A
    // standalone or flat-listed entry always does, because there the written path *is* what the
    // range covers.
    if view.text_in(unused.range) != Some(unused.path.as_str()) {
        return None;
    }

    let keyword = match unused.kind {
        ImportKind::ClassLike => "use ",
        ImportKind::Function => "use function ",
        ImportKind::Constant => "use const ",
    };
    let keyword_len = TextSize::try_from(keyword.len()).ok()?;
    let start = unused.range.start().checked_sub(keyword_len)?;
    // Only the first name in a comma list sits directly after the keyword; a later one sits
    // after `, ` instead. Requiring an exact match is what tells the two apart.
    if view.text_in(TextRange::new(start, unused.range.start())) != Some(keyword) {
        return None;
    }

    // The last path segment is what a plain import binds to; anything else means the source
    // wrote an alias, and the text after the path has to account for it.
    let last_segment = unused.path.as_str().rsplit('\\').next().unwrap_or("");
    let suffix = if unused.local.as_str() == last_segment {
        ";".to_owned()
    } else {
        format!(" as {};", unused.local)
    };
    let suffix_len = TextSize::try_from(suffix.len()).ok()?;
    let end = unused.range.end() + suffix_len;
    // Only the last name in a comma list or a group ends directly in `;`; an earlier one is
    // followed by `,` or `}` instead. Requiring an exact match is what tells those apart too --
    // together with the keyword check, this proves the import is the entire statement.
    if view.text_in(TextRange::new(unused.range.end(), end)) != Some(suffix.as_str()) {
        return None;
    }

    // Swallows the line's own trailing newline as well, so deleting the statement does not leave
    // a blank line where it stood. Left alone when there isn't one -- end of file, or another
    // statement sharing the line -- because a stray blank line is not a reason to refuse an
    // otherwise-proven edit.
    let end = match view.text_in(TextRange::new(end, end + TextSize::from(1))) {
        Some("\n") => end + TextSize::from(1),
        _ => end,
    };

    Some(TextEdit::replace(TextRange::new(start, end), ""))
}
