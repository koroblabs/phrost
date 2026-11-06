//! The ranges an editor grows a selection through, as a parent chain.
//!
//! The protocol asks about several positions at once -- one per cursor, for an editor with more than
//! one -- and wants each answer as a linked list from the innermost range outwards. The feature
//! produces a flat chain, and turning that into the nesting is all this layer does.
//!
//! Buffer-only, so it answers before the project has finished indexing.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_syntax_features::selection;

use crate::{convert, server::Backend};

impl Backend {
    pub(crate) async fn answer_selection_ranges(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let Some((_, file)) = self.file_of(&params.text_document.uri).await else {
            return Ok(None);
        };
        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.document_text(file) else {
            return Ok(None);
        };

        let mut answers = Vec::with_capacity(params.positions.len());
        for position in params.positions {
            let offset = convert::offset_of(&text, &lines, position);
            let steps = selection::ranges_at(&text, offset);

            // Built from the outside in, so each step becomes the parent of the one before it. The
            // feature hands them back innermost first, which is the order the client wants the head
            // of the chain in.
            let mut chain: Option<SelectionRange> = None;
            for step in steps.into_iter().rev() {
                chain = Some(SelectionRange {
                    range: convert::range_of(&text, &lines, step),
                    parent: chain.map(Box::new),
                });
            }

            // A position with no chain still needs an answer, because the protocol pairs answers
            // with positions by index: dropping one would shift every position after it onto the
            // wrong range.
            answers.push(chain.unwrap_or(SelectionRange {
                range: Range::new(position, position),
                parent: None,
            }));
        }

        Ok(Some(answers))
    }
}
