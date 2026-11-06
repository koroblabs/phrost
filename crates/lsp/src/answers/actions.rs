//! The fixes on offer at a place in the file.
//!
//! A pulled request: the client asks about a selection and gets a menu. Nothing here decides what is
//! on it -- see `phrost_analysis::actions` -- and nothing here waits for the index either, for the
//! same reason document highlight does not: a name in the open buffer is a name whether or not the
//! project has finished loading, and a menu that is empty during bootstrap teaches a person the
//! feature does not work.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_analysis::{ActionKind, ActionRequest, code_actions};

use crate::{convert, server::Backend};

impl Backend {
    pub(crate) async fn answer_code_actions(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<CodeActionResponse>> {
        let Some((_, file)) = self.file_of(&params.text_document.uri).await else {
            return Ok(None);
        };

        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.editable_text(file) else {
            return Ok(None);
        };
        let selection = phrost_model::TextRange::new(
            convert::offset_of(&text, &lines, params.range.start),
            convert::offset_of(&text, &lines, params.range.end),
        );

        let view = workspace.view(file);
        let only = requested_kinds(params.context.only.as_deref());
        let request = ActionRequest {
            view: &view,
            at: selection,
            only: &only,
            asked: workspace.asked_for(),
        };
        let uri = params.text_document.uri;
        let offered = code_actions(&request)
            .into_iter()
            .map(|action| {
                let edits = action
                    .edits
                    .into_iter()
                    .map(|edit| TextEdit {
                        range: convert::range_of(&text, &lines, edit.range),
                        new_text: edit.text,
                    })
                    .collect();
                CodeActionOrCommand::CodeAction(CodeAction {
                    title: action.title,
                    kind: Some(lsp_kind(action.kind)),
                    // Still not attached to the diagnostics the client sent. What is on offer is
                    // derived from the index, so an offer stands whether or not the client's list is
                    // current -- and a client that sent none still gets the menu. The code the
                    // action repairs travels on the action itself instead.
                    diagnostics: None,
                    edit: Some(WorkspaceEdit {
                        changes: Some([(uri.clone(), edits)].into_iter().collect()),
                        ..WorkspaceEdit::default()
                    }),
                    is_preferred: Some(action.preferred),
                    ..CodeAction::default()
                })
            })
            .collect::<Vec<_>>();

        Ok(Some(offered))
    }
}

/// The protocol's kind for one of ours.
fn lsp_kind(kind: ActionKind) -> CodeActionKind {
    match kind {
        ActionKind::QuickFix => CodeActionKind::QUICKFIX,
        ActionKind::Refactor => CodeActionKind::REFACTOR,
        ActionKind::OrganizeImports => CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
        ActionKind::FixAll => CodeActionKind::SOURCE_FIX_ALL,
    }
}

/// Which of our kinds a client's filter selects.
///
/// The protocol's kinds are a dotted hierarchy and a filter matches by prefix, so a client asking
/// for `refactor` means every refactor. Translating here rather than in `phrost-analysis` is what
/// keeps protocol types out of that crate. An absent filter means everything, which is not the same
/// as an empty one -- but an empty `only` from a client asking for nothing has nothing to offer
/// either way, so both read as "no restriction" and the menu is filtered by what applies.
fn requested_kinds(only: Option<&[CodeActionKind]>) -> Vec<ActionKind> {
    let Some(only) = only else {
        return Vec::new();
    };
    [
        (ActionKind::QuickFix, CodeActionKind::QUICKFIX),
        (ActionKind::Refactor, CodeActionKind::REFACTOR),
        (
            ActionKind::OrganizeImports,
            CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
        ),
        (ActionKind::FixAll, CodeActionKind::SOURCE_FIX_ALL),
    ]
    .into_iter()
    .filter(|(_, ours)| {
        only.iter()
            .any(|asked| ours.as_str().starts_with(asked.as_str()))
    })
    .map(|(kind, _)| kind)
    .collect()
}
