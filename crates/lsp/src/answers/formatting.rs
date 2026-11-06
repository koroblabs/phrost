//! Reprinting an open buffer in the project's house style.
//!
//! Answered from the buffer alone, so it is offered before the project is indexed: a formatter has
//! nothing to learn from a catalog, and refusing while indexing would mean format-on-save silently
//! doing nothing for the first seconds of a session.
//!
//! One edit, narrowed to what moved. The protocol allows replacing the whole document and that is
//! what a formatter naturally produces, but it is also what makes a client scroll to the top and
//! forget which regions were folded.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};

use crate::{convert, server::Backend};

impl Backend {
    pub(crate) async fn answer_formatting(&self, uri: &Uri) -> Result<Option<Vec<TextEdit>>> {
        let Some((_, file)) = self.file_of(uri).await else {
            return Ok(None);
        };
        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.editable_text(file) else {
            return Ok(None);
        };

        // `None` covers the case that decides whether this is safe on save: a file that does not
        // parse is left exactly as it is. Reformatting a half-written statement produces something
        // the author did not write and cannot undo in one step.
        let Some((range, replacement)) =
            phrost_syntax_features::format::format(&text, workspace.format_target())
        else {
            return Ok(None);
        };

        Ok(Some(vec![TextEdit {
            range: convert::range_of(&text, &lines, range),
            new_text: replacement,
        }]))
    }

    /// The same, for a selection.
    ///
    /// Answered from one whole-file reprint, cut into line runs and kept where they overlap what
    /// was asked about -- a fragment of PHP does not parse on its own, and a formatter's decision
    /// about a line depends on the construct around it. So this is a *subset* of what formatting
    /// the document would do rather than a second opinion about the selection.
    pub(crate) async fn answer_range_formatting(
        &self,
        uri: &Uri,
        range: Range,
    ) -> Result<Option<Vec<TextEdit>>> {
        let Some((_, file)) = self.file_of(uri).await else {
            return Ok(None);
        };
        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.editable_text(file) else {
            return Ok(None);
        };
        let asked = phrost_model::TextRange::new(
            convert::offset_of(&text, &lines, range.start),
            convert::offset_of(&text, &lines, range.end),
        );

        // `None` for a file that does not parse, for the reason the whole-document answer gives.
        let Some(hunks) =
            phrost_syntax_features::format::format_range(&text, workspace.format_target(), asked)
        else {
            return Ok(None);
        };

        Ok(Some(
            hunks
                .into_iter()
                .map(|(at, new_text)| TextEdit {
                    range: convert::range_of(&text, &lines, at),
                    new_text,
                })
                .collect(),
        ))
    }
}
