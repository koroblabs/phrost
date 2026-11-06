//! Going to a declaration, and finding everything that reaches it.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_analysis::{NavigationTarget, goto_definition, goto_external, goto_type_definition};
use std::sync::atomic::Ordering;

use crate::{
    answers::{positions::Positions, sweep_references},
    convert,
    server::Backend,
};

impl Backend {
    /// A navigation answer in the shape the client asked for.
    ///
    /// A `LocationLink` carries the span the answer was asked *from*, which is what lets an editor
    /// underline the word under the cursor -- and the protocol makes it opt-in, one bit per request
    /// under `textDocument.definition.linkSupport` and its three neighbours. A client that did not
    /// ask cannot deserialise one, so sending it anyway reads as goto answering nothing at all.
    /// This server sent links unconditionally until a probe declaring `linkSupport: false` got them
    /// back and had no field to read.
    ///
    /// One place rather than four, because four `::Link(...)` constructions is four chances to
    /// forget -- see §8.1 on two constructors of one rule. The plain form carries the *selection*
    /// range, which is the name: a client with no link support jumps to what it is given, and the
    /// declaration's whole extent would put the cursor on a brace.
    pub(crate) fn navigation(&self, links: Vec<LocationLink>) -> Option<GotoDefinitionResponse> {
        if links.is_empty() {
            return None;
        }
        if self.supports_definition_links.load(Ordering::Relaxed) {
            return Some(GotoDefinitionResponse::Link(links));
        }
        Some(GotoDefinitionResponse::Array(
            links
                .into_iter()
                .map(|link| Location {
                    uri: link.target_uri,
                    range: link.target_selection_range,
                })
                .collect(),
        ))
    }

    pub(crate) async fn answer_goto(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        self.jump_to(params, goto_definition).await
    }

    /// Where the *type* of the thing under the cursor is declared.
    ///
    /// A separate key in every editor and a separate question: on `$node` goto definition lands on
    /// the assignment and this lands on `Node`.
    pub(crate) async fn answer_goto_type_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        self.jump_to(params, goto_type_definition).await
    }

    /// The four lines every jump is: convert, ask, convert back, or nothing.
    ///
    /// Parameterised by the question rather than copied per question, so a new kind of jump is one
    /// method that names its own feature function.
    async fn jump_to(
        &self,
        params: GotoDefinitionParams,
        ask: fn(&phrost_view::IndexView<'_>, phrost_model::TextSize) -> Vec<NavigationTarget>,
    ) -> Result<Option<GotoDefinitionResponse>> {
        self.require_ready().await?;

        let position = params.text_document_position_params;
        // A configuration buffer, which the index holds nothing about. Asked first and disjoint
        // from what follows: a path is either one of those or one with a `FileId`, never both.
        if let Some(path) = self.config_of(&position.text_document.uri).await {
            return Ok(self
                .jump_in_config(&path, position.position)
                .await
                .and_then(|(clicked, found)| {
                    // The same shape every other jump here answers with: a client that reads one
                    // and not the other would follow a jump out of PHP and not one out of YAML.
                    self.navigation(vec![LocationLink {
                        origin_selection_range: Some(clicked),
                        target_uri: found.uri.clone(),
                        target_range: found.range,
                        target_selection_range: found.range,
                    }])
                }));
        }
        let Some((_, file)) = self.file_of(&position.text_document.uri).await else {
            return Ok(None);
        };

        let workspace = self.workspace.read().await;
        let Some((text, index)) = workspace.document_text(file) else {
            return Ok(None);
        };
        let offset = convert::offset_of(&text, &index, position.position);
        let view = workspace.view(file);

        // A key that names something declared outside the project's PHP -- a route, a permission.
        // Asked first, and not as a fallback: the two are disjoint, because a cursor is either on
        // such a key or it is not.
        if let Some(external) = goto_external(&view, offset)
            && let Some(uri) = Uri::from_file_path(&external.path)
        {
            let at = Position::new(external.line, external.character);
            // The name alone, so an editor selects the key rather than the block under it. Its
            // length in characters, which is its length in bytes: every key that reaches here is
            // written in a configuration file as plain ASCII.
            let end = Position::new(
                external.line,
                external.character + u32::try_from(external.key.len()).unwrap_or(0),
            );
            return Ok(self.navigation(vec![LocationLink {
                origin_selection_range: Some(convert::range_of(&text, &index, external.origin)),
                target_uri: uri,
                target_range: Range::new(at, end),
                target_selection_range: Range::new(at, end),
            }]));
        }

        let links = ask(&view, offset)
            .into_iter()
            .filter_map(|target| {
                let (target_text, target_index) = workspace.text_for_positions(target.file)?;
                let uri = Uri::from_file_path(workspace.path_of(target.file))?;

                Some(LocationLink {
                    // Converted against the *asking* file's text, which is the one this range is
                    // an offset into -- the target's is another file's.
                    origin_selection_range: target
                        .origin
                        .map(|origin| convert::range_of(&text, &index, origin)),
                    target_uri: uri,
                    target_range: convert::range_of(&target_text, &target_index, target.full_range),
                    target_selection_range: convert::range_of(
                        &target_text,
                        &target_index,
                        target.selection_range,
                    ),
                })
            })
            .collect::<Vec<_>>();

        Ok(self.navigation(links))
    }

    pub(crate) async fn answer_references(
        &self,
        params: ReferenceParams,
    ) -> Result<Option<Vec<Location>>> {
        self.require_ready().await?;

        let Some((_, file)) = self
            .file_of(&params.text_document_position.text_document.uri)
            .await
        else {
            return Ok(None);
        };

        // The target is decided from the cursor before anything is swept, so the search is for one
        // declaration rather than for a name -- two classes called `Repo` are two answers.
        let (target, needles) = {
            let workspace = self.workspace.read().await;
            let Some((text, lines)) = workspace.document_text(file) else {
                return Ok(None);
            };
            let offset = convert::offset_of(&text, &lines, params.text_document_position.position);
            let view = workspace.view(file);
            let target = phrost_analysis::reference_target(&view, offset);
            let needles = target
                .as_ref()
                .map(|target| target.sweep_query(&view))
                .unwrap_or_default();
            (target, needles)
        };
        let Some(target) = target else {
            return Ok(None);
        };
        // A read lock: the sweep analyses each candidate into a scratch buffer set of its own and
        // never touches the index, so nothing here needs to exclude another request. A target with
        // no search name never leaves its file, so there is nothing to sweep.
        let found = match needles.is_empty() {
            false => sweep_references(&self.workspace, &target, &needles).await,
            true => self.references_here(file, &target).await,
        };

        // The lock is taken again, and only to turn ranges into positions.
        let workspace = self.workspace.read().await;

        let mut positions = Positions::over(&workspace);
        let mut out: Vec<Location> = found
            .into_iter()
            .filter_map(|reference| positions.locate(reference.file, reference.range))
            .collect();

        // The places outside PHP -- see `config_sites_for`, which the lens and the rename read too.
        out.extend(
            super::config_sites_for(&workspace, &target)
                .into_iter()
                .filter_map(|(path, range)| positions.locate_path(&path, range)),
        );
        out.extend(super::catalog_declaration(&workspace, &target));

        Ok(Some(out))
    }
}
