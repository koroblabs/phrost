//! Walking a call graph, in either direction.
//!
//! Three requests for one feature, the same two-step shape the class hierarchy has: the client asks
//! which callable the cursor is on, then asks that item for its callers or its callees. An item is
//! all the follow-ups get, so it carries the declaration's identity -- `App\Repo::find`, or a
//! function's fully-qualified name -- and every step resolves that afresh.
//!
//! # Why the two directions cost so differently
//!
//! **Outgoing** reads one declaration's body. After the first step of the tree, that declaration is
//! in a file nobody has open, so the body has to be produced -- which the workspace does by reading
//! the file into a scratch buffer set and dropping it, exactly as a reference sweep does. One file
//! per request, retained for the length of the request.
//!
//! **Incoming** is a project-wide search, so it goes through the same sweep find-references does:
//! the lock is taken per candidate file and released before the runtime gets its turn, and a
//! cancelled request stops within one file of being asked to.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_analysis::{CallSubject, NavigationTarget};

use crate::{
    answers::{positions::Positions, sweep_project},
    convert,
    server::Backend,
};

impl Backend {
    pub(crate) async fn answer_call_hierarchy_head(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
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

        let Some(found) = phrost_analysis::call_hierarchy_head(&workspace.view(file), offset)
        else {
            return Ok(None);
        };

        let mut positions = Positions::over(&workspace);
        Ok(item_of(&mut positions, &found).map(|item| vec![item]))
    }

    pub(crate) async fn answer_incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        self.require_ready().await?;

        let Some(subject) = subject_of_item(&params.item) else {
            return Ok(None);
        };

        // What makes a file a candidate is a question about the class table, so it is asked
        // once and the lock let go of before the sweep takes its own.
        let query = {
            let workspace = self.workspace.read().await;
            subject.sweep_query(&workspace.project_view())
        };

        // A read lock per candidate file, released before each yield: the sweep analyses each file
        // into a scratch buffer set of its own and never touches the index, so nothing here needs to
        // exclude another request -- and an abort can land between two files.
        let found = sweep_project(&self.workspace, &query, |workspace, path| {
            let Some(file) = workspace.file_id(path) else {
                return Vec::new();
            };
            workspace
                .analysed(file, |view| phrost_analysis::incoming_calls(view, &subject))
                .unwrap_or_default()
                .into_iter()
                .map(|call| (file, call))
                .collect()
        })
        .await;

        // The lock is taken again, and only to turn ranges into positions.
        let workspace = self.workspace.read().await;
        let mut positions = Positions::over(&workspace);

        let out = found
            .into_iter()
            .filter_map(|(file, call)| {
                Some(CallHierarchyIncomingCall {
                    from: item_of(&mut positions, &call.caller)?,
                    from_ranges: ranges_in(&mut positions, file, &call.at),
                })
            })
            .collect();

        Ok(Some(out))
    }

    pub(crate) async fn answer_outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        self.require_ready().await?;

        let Some(subject) = subject_of_item(&params.item) else {
            return Ok(None);
        };

        let workspace = self.workspace.read().await;
        // Which file to read is a skeleton lookup, so it is answered before anything is analysed --
        // and the item may well name a declaration in a file this request is the first to touch.
        let Some(declared) =
            phrost_analysis::callable_declaration(&workspace.project_view(), &subject)
        else {
            return Ok(None);
        };
        let found = workspace
            .analysed(declared.file, |view| {
                phrost_analysis::outgoing_calls(view, &subject)
            })
            .unwrap_or_default();

        let mut positions = Positions::over(&workspace);
        let out = found
            .into_iter()
            .filter_map(|call| {
                Some(CallHierarchyOutgoingCall {
                    to: item_of(&mut positions, &call.callee)?,
                    // In the *caller's* file, which is where the call is written -- not in the
                    // callee's, which is where it lands.
                    from_ranges: ranges_in(&mut positions, declared.file, &call.at),
                })
            })
            .collect();

        Ok(Some(out))
    }
}

/// The subject an item stands for, as this server wrote it.
///
/// `None` for an item this server did not produce: the data field is opaque to the client, which
/// hands back whatever it was given, and a client that invented one is not owed an answer.
fn subject_of_item(item: &CallHierarchyItem) -> Option<CallSubject> {
    let key = item.data.as_ref()?.as_str()?;
    Some(CallSubject::of_key(key))
}

/// One declaration as the protocol's item, carrying its identity so a later step can ask about it.
fn item_of(positions: &mut Positions<'_>, target: &NavigationTarget) -> Option<CallHierarchyItem> {
    let subject = phrost_analysis::subject_of(target);
    Some(CallHierarchyItem {
        // The short name, because a tree shows one per row; the whole of it is in `detail`, where a
        // client shows it beside. A file's name is a path, whose last segment is separated by `/`
        // and not by PHP's `\`, so it is cut here rather than by `short_name`.
        name: match target.kind {
            phrost_model::SymbolKind::File => target
                .name
                .as_str()
                .rsplit('/')
                .next()
                .unwrap_or(target.name.as_str())
                .to_owned(),
            _ => phrost_model::short_name(target.name.as_str()).to_owned(),
        },
        kind: crate::answers::outline::symbol_kind(
            target.kind,
            phrost_model::short_name(target.name.as_str()),
        ),
        tags: None,
        detail: Some(subject.key()),
        uri: positions.uri(target.file)?,
        range: positions.range(target.file, target.full_range)?,
        selection_range: positions.range(target.file, target.selection_range)?,
        data: Some(serde_json::Value::String(subject.key())),
    })
}

/// The call sites inside one file, as client ranges.
///
/// A site whose range cannot be converted is dropped rather than failing the row: the row is still
/// a true statement about who calls what, and an editor showing it with one jump target missing is
/// better than an editor showing nothing.
fn ranges_in(
    positions: &mut Positions<'_>,
    file: phrost_model::FileId,
    at: &[phrost_model::TextRange],
) -> Vec<Range> {
    at.iter()
        .filter_map(|range| positions.range(file, *range))
        .collect()
}
