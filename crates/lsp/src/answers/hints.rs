//! Text drawn beside the code that the file does not contain.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};

use crate::{convert, server::Backend};

impl Backend {
    pub(crate) async fn answer_inlay_hints(
        &self,
        params: InlayHintParams,
    ) -> Result<Option<Vec<InlayHint>>> {
        self.require_ready().await?;

        let Some((_, file)) = self.file_of(&params.text_document.uri).await else {
            return Ok(None);
        };
        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.document_text(file) else {
            return Ok(None);
        };

        let range = phrost_model::TextRange::new(
            convert::offset_of(&text, &lines, params.range.start),
            convert::offset_of(&text, &lines, params.range.end),
        );
        let hints = phrost_analysis::inlay_hints(&workspace.view(file), range)
            .into_iter()
            .map(|hint| InlayHint {
                position: convert::position_of(&text, &lines, hint.offset),
                label: InlayHintLabel::String(hint.label),
                kind: Some(match hint.kind {
                    phrost_analysis::InlayKind::Parameter => InlayHintKind::PARAMETER,
                    phrost_analysis::InlayKind::Type => InlayHintKind::TYPE,
                }),
                text_edits: None,
                // A fence, for the reason the hover card uses one: the tooltip is what the label
                // could not fit, which is a type written over several lines of PHP.
                tooltip: hint.tooltip.map(|php| {
                    InlayHintTooltip::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("```php\n{php}\n```"),
                    })
                }),
                // A parameter name is drawn in front of the argument, where one written by hand
                // would be, so the space goes after it. A type is drawn where a declaration would
                // be -- tight against the name it describes, like the `: Repo` it stands in for.
                padding_left: None,
                padding_right: match hint.kind {
                    phrost_analysis::InlayKind::Parameter => Some(true),
                    phrost_analysis::InlayKind::Type => None,
                },
                data: None,
            })
            .collect();

        Ok(Some(hints))
    }
}
