//! Deleting every `use` line nothing in the file names, in one edit.
//!
//! The other half of [`super::unused_import`], and a different sort of thing. That one is offered
//! at a cursor and removes the import under it; this is asked of the *buffer* -- `source.organizeImports`
//! in the protocol -- which is what an editor binds to a menu entry and, often, to saving the file.
//! Hence [`ActionKind::OrganizeImports`], and hence a subject of `WholeFile`: the answer is the same wherever
//! the caret sits, so gating it on a range would only hide it.
//!
//! It removes exactly what the single-import offer can prove, and refuses exactly what that one
//! refuses: a grouped `use App\{Repo, Model};` or a flat `use App\Repo, App\Model;` shares its
//! statement with names the file may still use, and removing a line there would take them with it.
//! An offer that silently dropped a used import would be the worst thing in this directory -- a
//! change nobody read that breaks the file -- so a group is left whole and the person is told
//! nothing about it, which is the honest amount to say.
//!
//! One action and not several, even for one import: a person choosing "organize imports" has asked
//! for the file to be tidied, and being handed four menu entries instead is answering a different
//! question.

use super::{
    ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, unused_import::removal_edit,
};

pub(super) struct OrganizeImports;

impl Offer for OrganizeImports {
    fn kind(&self) -> ActionKind {
        ActionKind::OrganizeImports
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::WholeFile
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let view = sites.view();
        let mut edits = Vec::new();
        for unused in sites.all_imports() {
            if let Some(edit) = removal_edit(view, unused) {
                edits.push(edit);
            }
        }
        if edits.is_empty() {
            return;
        }

        // In source order, and non-overlapping by construction: each edit spans one whole `use`
        // statement, and two statements cannot share text. A client applies them together.
        edits.sort_by_key(|edit| edit.range.start());

        out.push(CodeAction {
            title: match edits.len() {
                1 => "Remove 1 unused import".to_owned(),
                many => format!("Remove {many} unused imports"),
            },
            edits,
            // Nothing about the file is wrong -- it runs -- so nothing here is what a keystroke
            // should apply unasked. A client that wants it on save asks for the kind by name.
            preferred: false,
            kind: ActionKind::OrganizeImports,
            fixes: None,
        });
    }
}
