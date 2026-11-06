//! What to show while a call is being written.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};

use crate::{convert, server::Backend};

impl Backend {
    pub(crate) async fn answer_signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> Result<Option<SignatureHelp>> {
        self.require_ready().await?;

        let position = params.text_document_position_params;
        let Some((_, file)) = self.file_of(&position.text_document.uri).await else {
            return Ok(None);
        };

        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.document_text(file) else {
            return Ok(None);
        };
        let offset = convert::offset_of(&text, &lines, position.position);

        let Some(help) = phrost_analysis::signature_help(&workspace.view(file), offset) else {
            return Ok(None);
        };

        Ok(Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: help.label,
                documentation: None,
                parameters: Some(
                    help.parameters
                        .into_iter()
                        .map(|parameter| ParameterInformation {
                            // The label as text rather than as offsets into the signature: the
                            // offsets would have to be byte ranges the client counts in UTF-16, and
                            // one wrong conversion highlights the wrong parameter.
                            label: ParameterLabel::Simple(parameter),
                            documentation: None,
                        })
                        .collect(),
                ),
                active_parameter: help.active.and_then(|index| u32::try_from(index).ok()),
            }],
            active_signature: Some(0),
            // Reported rather than left out, because a client that gets `None` keeps whatever it
            // highlighted last -- so a call whose position no longer means anything would go on
            // showing the parameter from the previous keystroke.
            active_parameter: help.active.and_then(|index| u32::try_from(index).ok()),
        }))
    }
}
