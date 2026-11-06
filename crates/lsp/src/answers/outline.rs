//! What a file declares, and finding a declaration by name across the project.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};

use crate::{answers::positions::Positions, convert, server::Backend};

impl Backend {
    pub(crate) async fn answer_document_symbols(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        self.require_ready().await?;

        let Some((path, file)) = self.file_of(&params.text_document.uri).await else {
            return Ok(None);
        };
        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.text_for_positions(file) else {
            return Ok(None);
        };
        drop(path);

        let symbols = phrost_analysis::document_symbols(&workspace.view(file))
            .into_iter()
            .map(|entry| document_symbol(&text, &lines, entry))
            .collect();
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    pub(crate) async fn answer_workspace_symbols(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<WorkspaceSymbolResponse>> {
        self.require_ready().await?;

        let workspace = self.workspace.read().await;
        let hits = phrost_analysis::workspace_symbols(&workspace.project_view(), &params.query);

        // One read per distinct file rather than per hit: a search can land a hundred times in the
        // same file, and turning a range into a client position needs that file's line map.
        let mut positions = Positions::over(&workspace);
        let out = hits
            .into_iter()
            .filter_map(|hit| {
                #[expect(
                    deprecated,
                    reason = "the struct's own deprecated field has to be given a value"
                )]
                Some(SymbolInformation {
                    kind: symbol_kind(hit.kind, &hit.label),
                    tags: hit.deprecated.then(|| vec![SymbolTag::DEPRECATED]),
                    name: hit.label,
                    // The struct's own field, which the protocol replaced with `tags` and which a
                    // client of either vintage reads: set neither way rather than both, because a
                    // server that sets only the old one is invisible to a modern client.
                    deprecated: None,
                    location: positions.locate(hit.file, hit.selection_range)?,
                    // The class a member was declared in. A client draws it beside the name, which
                    // is the difference between forty rows called `find` and forty rows a person
                    // can choose between.
                    container_name: hit.container,
                })
            })
            .collect();

        Ok(Some(WorkspaceSymbolResponse::Flat(out)))
    }
}

/// Maps a phrost symbol kind to the icon a client shows.
///
/// A separate function rather than a method on the kind: which glyph an editor draws is a protocol
/// concern, and nothing below this layer should have an opinion about it.
/// Converts one outline entry and its children.
fn document_symbol(
    text: &str,
    lines: &phrost_model::LineIndex,
    entry: phrost_analysis::SymbolEntry,
) -> DocumentSymbol {
    #[expect(
        deprecated,
        reason = "the struct's own deprecated fields have to be given"
    )]
    DocumentSymbol {
        kind: symbol_kind(entry.kind, &entry.label),
        tags: entry.deprecated.then(|| vec![SymbolTag::DEPRECATED]),
        name: entry.label,
        detail: entry.detail,
        deprecated: None,
        range: convert::range_of(text, lines, entry.full_range),
        selection_range: convert::range_of(text, lines, entry.selection_range),
        children: Some(
            entry
                .children
                .into_iter()
                .map(|child| document_symbol(text, lines, child))
                .collect(),
        ),
    }
}

pub(super) fn symbol_kind(kind: phrost_model::SymbolKind, name: &str) -> SymbolKind {
    use phrost_model::SymbolKind as Ours;

    // A constructor is a method the language calls, and the protocol has a kind of its own for it
    // -- which is a different icon in every tree an editor draws. Decided by the name because
    // that is what decides it in PHP: `__construct` is a method whose name makes it one, and there
    // is no modifier and no separate declaration to read instead.
    if kind == Ours::Method && name.eq_ignore_ascii_case("__construct") {
        return SymbolKind::CONSTRUCTOR;
    }

    match kind {
        Ours::Class => SymbolKind::CLASS,
        Ours::Interface => SymbolKind::INTERFACE,
        Ours::Trait => SymbolKind::INTERFACE,
        Ours::Enum => SymbolKind::ENUM,
        Ours::EnumCase => SymbolKind::ENUM_MEMBER,
        Ours::Function => SymbolKind::FUNCTION,
        Ours::Method => SymbolKind::METHOD,
        // `PROPERTY` and not `FIELD`, which are two kinds and two icons. The protocol keeps
        // `FIELD` for a struct member; what PHP declares is a property, which is the word the
        // language uses and the kind every other PHP server sends. Measured against intelephense on
        // one checkout, this was the only outline difference that was not a position: it answers
        // `property` where this answered `field` for the same six declarations.
        Ours::Property => SymbolKind::PROPERTY,
        Ours::Constant | Ours::ClassConstant => SymbolKind::CONSTANT,
        Ours::Namespace => SymbolKind::NAMESPACE,
        Ours::Variable => SymbolKind::VARIABLE,
        Ours::Parameter => SymbolKind::VARIABLE,
        // Unreachable: an outline lists what a file declares, and nothing declares a keyword or a
        // catalog key. The protocol has no kind for either, so `KEY` is as close as it gets.
        Ours::Keyword | Ours::CatalogKey => SymbolKind::KEY,
        // A caller that is a file rather than a declaration: a call written at a file's top level
        // or inside a closure the file returns. The protocol has the kind and every tree draws it.
        Ours::File => SymbolKind::FILE,
    }
}

#[cfg(test)]
mod tests {
    use super::symbol_kind;
    use lspkit_runtime::protocol::ls_types::SymbolKind;
    use phrost_model::SymbolKind as Ours;

    /// The one kind the protocol has that this crate's vocabulary does not, decided by the name the
    /// way PHP decides it.
    #[test]
    fn a_method_called_construct_is_a_constructor_and_its_neighbours_are_not() {
        assert_eq!(
            symbol_kind(Ours::Method, "__construct"),
            SymbolKind::CONSTRUCTOR
        );
        // PHP looks method names up case-insensitively, and a class writing `__Construct` declares
        // the same constructor.
        assert_eq!(
            symbol_kind(Ours::Method, "__Construct"),
            SymbolKind::CONSTRUCTOR
        );
        // Every other magic method is an ordinary method as far as a tree is concerned, and so is a
        // *function* of that name -- which PHP allows and which is not a constructor.
        assert_eq!(symbol_kind(Ours::Method, "__toString"), SymbolKind::METHOD);
        assert_eq!(
            symbol_kind(Ours::Function, "__construct"),
            SymbolKind::FUNCTION
        );
    }
}
