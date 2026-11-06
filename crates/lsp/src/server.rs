use std::{
    ops::Deref,
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};

use lspkit_runtime::protocol::{Client, LanguageServer, jsonrpc::Result, ls_types::*};
use phrost_model::FileId;
use phrost_stubs::StubSet;

use crate::{bootstrap::Pass, documents::Publish, session::Session, workspace::Workspace};

/// The protocol surface, and a handle to the state behind it.
///
/// Every handler does the same four things: convert the request, guard it, ask the analysis layer,
/// convert the answer. Anything more than that in here is a sign something belongs a layer down.
///
/// A handle rather than the state itself because indexing does not run on the handler that asks for
/// it: `initialized` spawns it, `tokio::spawn` wants a `'static` future, and a `&self` borrowed from
/// the transport cannot make one. Cloning a `Backend` is one atomic increment and every clone is the
/// same server, so the [`Deref`] below keeps `self.workspace` and the rest reading as they did.
#[derive(Clone, Debug)]
pub(crate) struct Backend {
    session: Arc<Session>,
}

impl Deref for Backend {
    type Target = Session;

    fn deref(&self) -> &Session {
        &self.session
    }
}

impl Backend {
    /// A server over project code and the PHP standard library.
    ///
    /// Installing the stubs is the one thing done before any request arrives, because every project
    /// needs them and none declares them. A failure here is not fatal — the project's own code still
    /// resolves — but it is reported, since the symptom is the whole standard library reading as
    /// undeclared.
    pub(crate) fn new(client: Client) -> Self {
        let workspace = match StubSet::install() {
            Ok(stubs) => Workspace::new(stubs),
            Err(error) => Workspace::without_stubs(format!("PHP stubs unavailable: {error}")),
        };
        Self::over(client, workspace)
    }

    /// A server over a workspace built elsewhere. What tests use, to stay off the real cache.
    pub(crate) fn over(client: Client, workspace: Workspace) -> Self {
        Self {
            session: Arc::new(Session::over(client, workspace)),
        }
    }

    /// Resolves a document URI to an indexed file.
    ///
    /// `None` means the client is asking about a document this server has never seen, which is a
    /// normal thing for it to do — a scratch buffer, or a file outside the project.
    pub(crate) async fn file_of(&self, uri: &Uri) -> Option<(PathBuf, FileId)> {
        let path = uri.to_file_path()?.to_path_buf();
        let file = self.workspace.read().await.file_id(&path)?;
        Some((path, file))
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // A workspace folder is the modern form; `root_uri` is what older clients send. Taking the
        // first folder is enough until there is a reason to index several roots as one project.
        let root = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first())
            .and_then(|folder| folder.uri.to_file_path())
            .map(|path| path.to_path_buf())
            .or_else(|| {
                #[allow(deprecated)]
                params
                    .root_uri
                    .as_ref()
                    .and_then(|uri| uri.to_file_path())
                    .map(|path| path.to_path_buf())
            });

        // Before the root, because fixing the root is what reads `phrost.toml` and merges the two --
        // and this is the source that wins. What the client sends is the object it was given
        // verbatim; in Zed that is `lsp.phrost.initialization_options` from `.zed/settings.json`.
        //
        // Read once, here. There is no `workspace/didChangeConfiguration` handler on purpose: that
        // notification carries a *different* setting in every client that sends it -- Zed's is
        // `lsp.phrost.settings`, which is not what a person was told to write -- so treating it as
        // this source would mean honouring a key nobody set and ignoring the one they did. Changing
        // these settings takes a language-server restart.
        {
            let mut workspace = self.workspace_write().await;
            workspace.set_editor_config(params.initialization_options);
            if let Some(root) = root {
                workspace.set_root(root);
            }
        }

        // Which token types the client can paint. Read before the capabilities are composed,
        // because the legend this server declares is the intersection -- a client cannot render a
        // type it did not name, and the colouring response is the largest message this server
        // sends.
        if let Some(tokens) = params
            .capabilities
            .text_document
            .as_ref()
            .and_then(|text| text.semantic_tokens.as_ref())
        {
            let _ = self.understood_tokens.set(tokens.token_types.to_vec());
        }

        self.supports_snippets.store(
            params
                .capabilities
                .text_document
                .as_ref()
                .and_then(|document| document.completion.as_ref())
                .and_then(|completion| completion.completion_item.as_ref())
                .and_then(|item| item.snippet_support)
                .unwrap_or(false),
            Ordering::Relaxed,
        );

        // Type hierarchy has no static server capability -- the specification gives it none, and a
        // client issues the requests only if the server *registers* for them. So the client's own
        // support is recorded here and the registration happens at `initialized`.
        self.supports_type_hierarchy.store(
            params
                .capabilities
                .text_document
                .as_ref()
                .and_then(|document| document.type_hierarchy.as_ref())
                .and_then(|hierarchy| hierarchy.dynamic_registration)
                .unwrap_or(false),
            Ordering::Relaxed,
        );

        self.supports_file_watching.store(
            params
                .capabilities
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.did_change_watched_files.as_ref())
                .and_then(|watched| watched.dynamic_registration)
                .unwrap_or(false),
            Ordering::Relaxed,
        );
        self.supports_rooted_watch.store(
            params
                .capabilities
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.did_change_watched_files.as_ref())
                .and_then(|watched| watched.relative_pattern_support)
                .unwrap_or(false),
            Ordering::Relaxed,
        );
        self.supports_lens_refresh.store(
            params
                .capabilities
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.code_lens.as_ref())
                .and_then(|lens| lens.refresh_support)
                .unwrap_or(false),
            Ordering::Relaxed,
        );
        self.supports_definition_links.store(
            params
                .capabilities
                .text_document
                .as_ref()
                .is_some_and(|document| {
                    let asked = [
                        document
                            .definition
                            .as_ref()
                            .and_then(|one| one.link_support),
                        document
                            .declaration
                            .as_ref()
                            .and_then(|one| one.link_support),
                        document
                            .type_definition
                            .as_ref()
                            .and_then(|one| one.link_support),
                        document
                            .implementation
                            .as_ref()
                            .and_then(|one| one.link_support),
                    ];
                    asked.into_iter().flatten().any(|one| one)
                }),
            Ordering::Relaxed,
        );
        self.progress.negotiate(&params.capabilities);

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "phrost".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            capabilities: ServerCapabilities {
                // Full text on every change, for now. Incremental sync is a real win on large
                // files, but only once there is an incremental parse to feed it; until then it
                // would mean reassembling the whole text anyway, with more ways to get it wrong.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                // Answered from the buffer alone, so unlike everything else here it is offered
                // before the project is indexed.
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                // Also from the buffer alone: a formatter has nothing to learn from the index, and
                // refusing while indexing would mean format-on-save doing nothing for the first
                // seconds of a session.
                document_formatting_provider: Some(OneOf::Left(true)),
                // The same reprint, filtered to the selection -- see `format_range`. Advertised
                // separately because a client asks for one or the other and offers "format
                // selection" only when this is here.
                document_range_formatting_provider: Some(OneOf::Left(true)),
                // Resolving, because the listing is an outline walk and the count is a sweep: a
                // file declares tens of things and a client asks for the counts of the few it is
                // about to paint. See `answers::lenses`.
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(true),
                }),
                // PSR-4 makes a class's name a function of its file's path, so a client that is
                // about to move one asks here what else has to change. Only PHP, and only
                // `willRename`: the edits belong in the same undo step as the move, and there is
                // nothing left to say once it has happened.
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: None,
                    file_operations: Some(WorkspaceFileOperationsServerCapabilities {
                        will_rename: Some(FileOperationRegistrationOptions {
                            filters: vec![FileOperationFilter {
                                scheme: Some("file".to_owned()),
                                pattern: FileOperationPattern {
                                    glob: "**/*.php".to_owned(),
                                    matches: Some(FileOperationPatternKind::File),
                                    options: None,
                                },
                            }],
                        }),
                        ..WorkspaceFileOperationsServerCapabilities::default()
                    }),
                }),
                // Also answered from the buffer alone, so also offered before the project is
                // indexed. No delta support: that means holding the previous answer per document,
                // and a stale one miscolours every line after an edit.
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: crate::answers::semantic_token_legend(
                                self.understood_tokens.get().map(Vec::as_slice),
                            ),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: Some(true),
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                        },
                    ),
                ),
                inlay_hint_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                // With a prepare step, so the editor asks what will be replaced before it asks for
                // a new name -- and gets a refusal on something that cannot be renamed while there
                // is still nothing to undo.
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                })),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // `(` and `,` retrigger it, which is what makes the highlighted parameter follow
                // the cursor as arguments are typed rather than only when the call is opened.
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_owned(), ",".to_owned()]),
                    retrigger_characters: Some(vec![",".to_owned()]),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                document_highlight_provider: Some(OneOf::Left(true)),
                // Plain `true` rather than a declared list of kinds. A client that filters by kind
                // asks for `quickfix` and gets everything here; declaring the list would mean
                // keeping two places in step for no answer that changes.
                // The kinds by name, because a client deciding whether to ask this server for
                // `source.fixAll` on save reads the list rather than trying.
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::REFACTOR,
                            CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
                            CodeActionKind::SOURCE_FIX_ALL,
                        ]),
                        ..CodeActionOptions::default()
                    },
                )),
                selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
                // Where the type of the thing under the cursor is declared, which is a different
                // question from where the thing is: on `$node` one lands on the assignment and the
                // other on `Node`.
                type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
                // PHP has no separate declaration -- no headers, no forward declarations -- so this
                // is the same question as definition, answered rather than refused because editors
                // bind it to its own key and a refusal reads as a broken server.
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                // Which classes inherit from the one under the cursor. Most useful exactly where a
                // codebase is written against interfaces, which is every framework one.
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                // Who calls this, and what does this call. Unlike the type hierarchy beside it,
                // this one has a static capability -- the specification gives it one -- so it needs
                // no registration at `initialized` and reaches clients that never offered dynamic
                // registration.
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    // The characters that finish an access operator or a namespace segment. A
                    // client asks on its own as letters are typed; these are the ones it would
                    // otherwise treat as word boundaries and stay silent for.
                    trigger_characters: Some(vec![">".to_owned(), ":".to_owned(), "\\".to_owned()]),
                    // The list is sent without declarations and the client asks for the one it is
                    // showing. Two hundred declarations built per keystroke, of which a person reads
                    // one, is the whole reason this exists.
                    resolve_provider: Some(true),
                    ..CompletionOptions::default()
                }),
                ..ServerCapabilities::default()
            },
            // The clangd UTF-8 offsets extension. Left unset, which means the standard UTF-16
            // columns -- the only encoding every client supports. Negotiating UTF-8 would let the
            // conversion in `convert` disappear for clients that accept it, but only after there is
            // a test proving both paths agree; one silently wrong encoding is worse than a
            // conversion that is always applied.
            offset_encoding: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        // Started, not awaited. See `Backend::index_in_the_background` for what awaiting it here
        // cost.
        self.index_in_the_background(Pass::Startup);
    }

    // Every request below is four lines by design: convert, guard, call, convert back. The work is
    // in `answers`, a file per kind of answer, so adding one does not grow this file.
    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        self.apply_watched_files(params).await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        self.answer_goto(params).await
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        self.answer_references(params).await
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        self.answer_signature_help(params).await
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        self.answer_hover(params).await
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.answer_completion(params).await
    }

    async fn completion_resolve(&self, item: CompletionItem) -> Result<CompletionItem> {
        self.answer_completion_resolve(item).await
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        self.answer_document_symbols(params).await
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<WorkspaceSymbolResponse>> {
        self.answer_workspace_symbols(params).await
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        self.answer_folding_ranges(params).await
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        self.answer_code_actions(params).await
    }

    async fn goto_type_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        self.answer_goto_type_definition(params).await
    }

    async fn goto_implementation(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        self.answer_implementations(params).await
    }

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        self.answer_call_hierarchy_head(params).await
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        self.answer_incoming_calls(params).await
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        self.answer_outgoing_calls(params).await
    }

    async fn prepare_type_hierarchy(
        &self,
        params: TypeHierarchyPrepareParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        self.answer_hierarchy_head(params).await
    }

    async fn supertypes(
        &self,
        params: TypeHierarchySupertypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        self.answer_supertypes(params).await
    }

    async fn subtypes(
        &self,
        params: TypeHierarchySubtypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        self.answer_subtypes(params).await
    }

    async fn goto_declaration(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        self.answer_goto(params).await
    }

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        self.answer_selection_ranges(params).await
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        // The client's own tab size and insert-spaces are ignored on purpose. A PHP house style is
        // more than an indent width -- quote style, brace placement, trailing commas -- and honouring
        // half of it from the editor while taking the other half from the style would produce a file
        // that matches neither.
        self.answer_formatting(&params.text_document.uri).await
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        // The client's tab size is ignored for the reason `formatting` gives above.
        self.answer_range_formatting(&params.text_document.uri, params.range)
            .await
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        self.answer_code_lens(&params.text_document.uri).await
    }

    async fn code_lens_resolve(&self, params: CodeLens) -> Result<CodeLens> {
        self.answer_code_lens_resolve(params).await
    }

    async fn will_rename_files(&self, params: RenameFilesParams) -> Result<Option<WorkspaceEdit>> {
        self.answer_will_rename(params).await
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        self.answer_semantic_tokens(&params.text_document.uri, None)
            .await
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let answer = self
            .answer_semantic_tokens(&params.text_document.uri, Some(params.range))
            .await?;
        Ok(answer.map(|answer| match answer {
            SemanticTokensResult::Tokens(tokens) => SemanticTokensRangeResult::Tokens(tokens),
            SemanticTokensResult::Partial(partial) => SemanticTokensRangeResult::Partial(partial),
        }))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        self.answer_document_highlights(params).await
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        self.answer_inlay_hints(params).await
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        self.answer_prepare_rename(params).await
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        self.answer_rename(params).await
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document = params.text_document;
        let Some(path) = document.uri.to_file_path() else {
            return;
        };
        // At once: nobody is typing yet, and the first answer is the one a reader is waiting for.
        self.store_document(
            &path,
            &document.uri,
            document.text,
            document.version,
            Publish::Now,
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(path) = params.text_document.uri.to_file_path() else {
            return;
        };
        // Full sync, so the last change carries the whole document.
        let Some(change) = params.content_changes.into_iter().next_back() else {
            return;
        };
        self.store_document(
            &path,
            &params.text_document.uri,
            change.text,
            params.text_document.version,
            Publish::WhenQuiet,
        )
        .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let Some(path) = params
            .text_document
            .uri
            .to_file_path()
            .map(|path| path.to_path_buf())
        else {
            return;
        };
        self.workspace_write().await.close_document(&path);

        // What the disk says now, rather than nothing. A client keeps showing the last set it was
        // sent, so a closed buffer used to be told "none" -- and with the whole project reported, a
        // problem in the file should stay in the panel after its tab closes, as it would for a file
        // never opened, while a buffer's findings that were fixed on disk go. "None" still, for a
        // file this server does not check or when the project is not being checked.
        let found = match self.indexed.load(Ordering::Relaxed)
            && self.workspace.read().await.diagnoses_project()
        {
            true => Self::findings_from_disk(&self.workspace, &path)
                .await
                .unwrap_or_default(),
            false => Vec::new(),
        };
        self.note_project_findings(&path, !found.is_empty());
        self.client
            .publish_diagnostics(params.text_document.uri, found, None)
            .await;
    }
}
