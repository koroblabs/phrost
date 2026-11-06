//! Which classes inherit from the one under the cursor.
//!
//! No sweep and no file read: the class table already holds resolved supertypes for every file in the
//! project, so the answer is a pass over it. That is what makes this affordable at all -- the
//! grep-and-analyse shape references uses would have to read thirty thousand files here, because the
//! implementers of a framework's interface are in the framework and references deliberately looks
//! only at code the project owns.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_analysis::goto_implementation;

use crate::{convert, server::Backend};

impl Backend {
    pub(crate) async fn answer_implementations(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
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

        let links = goto_implementation(&workspace.view(file), offset)
            .into_iter()
            .filter_map(|found| {
                let (target_text, target_lines) = workspace.text_for_positions(found.file)?;
                let uri = Uri::from_file_path(workspace.path_of(found.file))?;
                Some(LocationLink {
                    // The span the reader is on, so a client underlines the name rather than
                    // working out where it ends by its own idea of a word.
                    origin_selection_range: found
                        .origin
                        .map(|origin| convert::range_of(&text, &lines, origin)),
                    target_uri: uri,
                    target_range: convert::range_of(&target_text, &target_lines, found.full_range),
                    target_selection_range: convert::range_of(
                        &target_text,
                        &target_lines,
                        found.selection_range,
                    ),
                })
            })
            .collect::<Vec<_>>();

        if links.is_empty() {
            return Ok(None);
        }
        Ok(self.navigation(links))
    }
}
