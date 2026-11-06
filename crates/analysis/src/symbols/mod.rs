//! Listing what exists: the outline of one file, and a search across the project.
//!
//! The two are one feature because they answer one question at two scales -- "what is declared" --
//! and because a client shows them in the same widget with the same wording.

use phrost_model::{SymbolKind, TextRange};
use phrost_view::IndexView;

/// One entry in an outline or a search result.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolEntry {
    /// What a reader sees: the short name for an outline entry, the qualified one for a search hit.
    pub label: String,
    pub kind: SymbolKind,
    /// The declaration as PHP, beside the name.
    pub detail: Option<String>,
    /// The name alone -- what selecting the entry should reveal.
    pub selection_range: TextRange,
    /// The whole declaration, for folding and highlighting.
    pub full_range: TextRange,
    pub children: Vec<SymbolEntry>,
    /// Whether the declaration says it is deprecated -- an editor draws the row struck through.
    pub deprecated: bool,
}

/// Everything the current file declares, classes holding their members.
pub fn document_symbols(view: &IndexView<'_>) -> Vec<SymbolEntry> {
    view.outline().into_iter().map(entry_of).collect()
}

/// One search hit, with the file it is in.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SearchHit {
    pub file: phrost_model::FileId,
    /// What a reader picks the row out by.
    ///
    /// Qualified for a top-level declaration, because a search result is read out of context and
    /// `Repo` alone says nothing about which of the four is meant -- and because the qualified name
    /// is how a person writes a class. A member's is the bare name, because `App\Repo::find` is not
    /// a name PHP resolves and nobody types one: where it was declared belongs in [`Self::container`]
    /// instead, which is the field the protocol has for exactly this.
    pub label: String,
    /// The class a member was declared in. `None` for a top-level declaration.
    pub container: Option<String>,
    pub kind: SymbolKind,
    pub selection_range: TextRange,
    pub full_range: TextRange,
    /// Whether the declaration says it is deprecated -- an editor draws the row struck through.
    pub deprecated: bool,
}

/// How many hits one search returns.
///
/// A client shows a couple of dozen and filters as the query narrows. The cap is about not building
/// a list nobody reads -- and the protocol, unlike completion's `isIncomplete`, has no way to say a
/// symbol search was cut short. So the cap is applied after ranking rather than during collection:
/// what comes back is the best page, not the first one found.
const SEARCH_LIMIT: usize = 128;

/// Declarations whose name starts with `query`, project-wide.
pub fn workspace_symbols(view: &IndexView<'_>, query: &str) -> Vec<SearchHit> {
    view.search_symbols(query.trim(), SEARCH_LIMIT)
        .into_iter()
        // A stub's internal marker interface is a real declaration the standard library ships,
        // but its whole point is that nobody writes it -- so it has no business turning up as a
        // search result either. See [`crate::render::is_stub_internal`].
        .filter(|location| !crate::render::is_stub_internal(location.fqn.as_str()))
        .map(|location| SearchHit {
            file: location.file,
            label: location.fqn.as_str().to_owned(),
            container: location
                .container
                .as_ref()
                .map(|owner| owner.as_str().to_owned()),
            kind: location.kind,
            selection_range: location.name_range,
            full_range: location.full_range,
            deprecated: location.deprecated,
        })
        .collect()
}

fn entry_of(node: phrost_view::OutlineNode) -> SymbolEntry {
    let label = node.name.into_string();
    // A constant's rendered declaration is its name, and repeating it beside itself is noise in a
    // list a person is scanning.
    let detail = node
        .detail
        .map(Into::into)
        .filter(|detail: &String| detail != &label);

    SymbolEntry {
        label,
        kind: node.kind,
        detail,
        selection_range: node.name_range,
        full_range: node.full_range,
        children: node.children.into_iter().map(entry_of).collect(),
        deprecated: node.deprecated,
    }
}
