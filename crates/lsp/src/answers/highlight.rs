//! Every place in this file that means the same thing as what the cursor is on.
//!
//! The same question references answers, restricted to the buffer -- which is what makes it a
//! different feature: it fires from cursor movement rather than from a command, so it can only ever
//! read the file already open. No sweep, no candidate list, no lock held across files.
//!
//! And no waiting for the project. A local variable's occurrences are in the buffer, so the answer
//! exists from the moment the file is opened -- refusing until the index is built turned every
//! cursor movement during a 2.5-second bootstrap into a failed request, five hundred of them in one
//! session of somebody's log. What needs the index is a class or a member, and that resolves to
//! nothing until it can, which is a quiet no rather than an error.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_analysis::{OccurrenceKind, ReferenceTarget, references_in};

use crate::{convert, server::Backend};

impl Backend {
    pub(crate) async fn answer_document_highlights(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let position = params.text_document_position_params;
        let Some((_, file)) = self.file_of(&position.text_document.uri).await else {
            return Ok(None);
        };

        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.document_text(file) else {
            return Ok(None);
        };
        let offset = convert::offset_of(&text, &lines, position.position);

        let view = workspace.view(file);
        let Some(target) = phrost_analysis::reference_target(&view, offset) else {
            return Ok(None);
        };

        // A local knows which of its occurrences write, so it says so: an editor draws a write
        // differently, and that is most of what makes the feature worth having on a variable.
        // Everything else is `Text`, because for a class or a member the distinction is not recorded
        // and claiming `Read` for a declaration would colour the wrong one.
        if let ReferenceTarget::Local { name, at } = &target {
            return Ok(Some(
                view.local_occurrences(*at, name.as_str())
                    .into_iter()
                    .map(|occurrence| DocumentHighlight {
                        range: convert::range_of(&text, &lines, occurrence.range),
                        kind: Some(match occurrence.kind {
                            OccurrenceKind::Read => DocumentHighlightKind::READ,
                            // A compound assignment does both, and `WRITE` is the half a reader is
                            // scanning for: the protocol has no kind that says both at once.
                            OccurrenceKind::Write | OccurrenceKind::ReadWrite => {
                                DocumentHighlightKind::WRITE
                            }
                        }),
                    })
                    .collect(),
            ));
        }

        Ok(Some(
            references_in(&view, &target)
                .into_iter()
                .map(|range| DocumentHighlight {
                    range: convert::range_of(&text, &lines, range),
                    kind: Some(DocumentHighlightKind::TEXT),
                })
                .collect(),
        ))
    }
}
