//! Which ranges of an open buffer the editor may collapse.
//!
//! The one answer this server gives without a project. It reads the buffer's text and nothing else,
//! so it does not wait for indexing and it does not read from disk -- which makes it the one feature
//! that works in the second and a half before the rest of the server can say anything at all.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_model::LineIndex;
use phrost_syntax_features::folding::{FoldKind, FoldRange};

use crate::{convert, server::Backend};

impl Backend {
    pub(crate) async fn answer_folding_ranges(
        &self,
        params: FoldingRangeParams,
    ) -> Result<Option<Vec<FoldingRange>>> {
        let Some((_, file)) = self.file_of(&params.text_document.uri).await else {
            return Ok(None);
        };
        let workspace = self.workspace.read().await;
        // The open buffer only. Falling back to the file on disk would answer about text the editor
        // has already changed, and folds drawn against the wrong revision land on the wrong lines.
        let Some((text, lines)) = workspace.editable_text(file) else {
            return Ok(None);
        };

        let folds = phrost_syntax_features::folding::ranges(&text)
            .into_iter()
            .map(|fold| folding_range(&text, &lines, fold))
            .collect();
        Ok(Some(folds))
    }
}

fn folding_range(text: &str, lines: &LineIndex, fold: FoldRange) -> FoldingRange {
    let range = convert::range_of(text, lines, fold.range);
    FoldingRange {
        start_line: range.start.line,
        start_character: Some(range.start.character),
        end_line: range.end.line,
        end_character: Some(range.end.character),
        kind: kind_of(fold.kind),
        collapsed_text: None,
    }
}

/// The protocol's name for a kind of fold.
///
/// `None` for a delimiter pair, which is not an omission: the protocol has three named kinds and
/// expects no kind at all for everything else.
fn kind_of(kind: FoldKind) -> Option<FoldingRangeKind> {
    match kind {
        FoldKind::Block => None,
        FoldKind::Imports => Some(FoldingRangeKind::Imports),
        FoldKind::Comment => Some(FoldingRangeKind::Comment),
        FoldKind::Region => Some(FoldingRangeKind::Region),
    }
}
