//! Walking up and down a class hierarchy.
//!
//! Three requests for one feature: the client asks which declaration the cursor is on, then asks that
//! item for its supertypes or its subtypes, as many times as a person keeps expanding the tree. So an
//! item has to carry enough to be resolved later, and what it carries is the fully-qualified name --
//! the same identity every other cross-file answer here is keyed by.
//!
//! Both directions were already there. Upwards is the class's own header; downwards is the pass over
//! the class table that goto-implementation uses. Nothing new is stored for either.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_analysis::{NavigationTarget, hierarchy_head, subtypes_of, supertypes_of};

use crate::{convert, server::Backend};

impl Backend {
    pub(crate) async fn answer_hierarchy_head(
        &self,
        params: TypeHierarchyPrepareParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
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

        let Some(found) = hierarchy_head(&workspace.view(file), offset) else {
            return Ok(None);
        };
        Ok(item_of(&workspace, found).map(|item| vec![item]))
    }

    pub(crate) async fn answer_supertypes(
        &self,
        params: TypeHierarchySupertypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        self.walk(params.item, supertypes_of).await
    }

    pub(crate) async fn answer_subtypes(
        &self,
        params: TypeHierarchySubtypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        self.walk(params.item, subtypes_of).await
    }

    /// One step of the tree, in whichever direction.
    ///
    /// Parameterised by the question rather than written twice: the two differ only in which list they
    /// ask for, and a copy would be two places to keep the item encoding in step.
    async fn walk(
        &self,
        item: TypeHierarchyItem,
        ask: fn(&phrost_view::IndexView<'_>, &str) -> Vec<NavigationTarget>,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        self.require_ready().await?;

        // The name the item was created with. A client hands back what it was given, so this is the
        // one place the encoding has to agree with itself.
        let Some(fqn) = item.data.as_ref().and_then(|data| data.as_str()) else {
            return Ok(None);
        };
        let workspace = self.workspace.read().await;
        let found = ask(&workspace.project_view(), fqn)
            .into_iter()
            .filter_map(|target| item_of(&workspace, target))
            .collect::<Vec<_>>();

        Ok(Some(found))
    }
}

/// One declaration as the protocol's item, carrying its name so a later step can ask about it.
fn item_of(
    workspace: &crate::workspace::Workspace,
    target: NavigationTarget,
) -> Option<TypeHierarchyItem> {
    let (text, lines) = workspace.text_for_positions(target.file)?;
    let uri = Uri::from_file_path(workspace.path_of(target.file))?;

    Some(TypeHierarchyItem {
        // The short name, because a tree shows one per row and a fully-qualified name would push the
        // rest of the row off the side. The whole name is in `detail`, where a client shows it beside.
        name: phrost_model::short_name(target.name.as_str()).to_owned(),
        kind: crate::answers::outline::symbol_kind(
            target.kind,
            phrost_model::short_name(target.name.as_str()),
        ),
        tags: None,
        detail: Some(target.name.as_str().to_owned()),
        uri,
        range: convert::range_of(&text, &lines, target.full_range),
        selection_range: convert::range_of(&text, &lines, target.selection_range),
        data: Some(serde_json::Value::String(target.name.as_str().to_owned())),
    })
}
