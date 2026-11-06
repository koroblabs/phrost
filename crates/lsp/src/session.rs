//! What a server holds for as long as a project is open.
//!
//! Split out from the handlers because indexing no longer runs on the handler that asks for it.
//! `tokio::spawn` wants a `'static` future and `LanguageServer` hands a handler `&self`, so the
//! state a background pass reaches has to be something a task can *own* -- which is what this is,
//! and why [`crate::Backend`] is now a handle to one rather than the thing itself.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use lspkit_runtime::panics::PoisoningWrite;
use lspkit_runtime::progress::Progress;
use lspkit_runtime::protocol::{
    Client,
    jsonrpc::{Error, Result},
    ls_types::SemanticTokenType,
};
use lspkit_runtime::reported::Reported;
use tokio::sync::{Mutex, RwLock, watch};

use crate::workspace::{Workspace, composer::ComposerStamp};

/// Everything a request and a background indexing pass both reach.
#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) client: Client,
    pub(crate) workspace: Arc<RwLock<Workspace>>,
    /// Whether the client said it can be asked to watch files.
    ///
    /// There is no static capability for `workspace/didChangeWatchedFiles`; a server has to register
    /// for it, and may only do so if the client advertised dynamic registration. A client that did
    /// not will never report a change made outside the editor, and the index will drift after a
    /// `git switch` — which is worth logging rather than failing silently.
    pub(crate) supports_file_watching: AtomicBool,
    /// Whether a watch can be rooted at a directory, rather than spelled as a glob over the project.
    ///
    /// The difference is whether an ignored directory is watched at all. Zed answers a `**` glob
    /// from the tree it already keeps, which holds no entry under a gitignored `vendor/`, and loads
    /// a directory into that tree only when a watch names it from its root.
    pub(crate) supports_rooted_watch: AtomicBool,
    /// Whether a task is already waiting for a composer run to finish.
    pub(crate) waiting_for_composer: AtomicBool,
    /// The composer stamp the last re-read of the project was started against.
    ///
    /// So the task waiting for a run can tell whether a re-read already happened after the run was
    /// recorded -- the editor that reports the record starts one at once, and a second would read
    /// the same tree again.
    pub(crate) reconfigured_for: std::sync::Mutex<Option<ComposerStamp>>,
    /// Whether the client will accept a registration for the type-hierarchy requests, which is the
    /// only way to offer them: the specification defines no server capability for them.
    pub(crate) supports_type_hierarchy: AtomicBool,
    /// Whether the client reads `$0` in an inserted string as a cursor.
    ///
    /// Asked rather than assumed: a client that does not gets the two characters put in its buffer.
    pub(crate) supports_snippets: AtomicBool,
    /// The semantic token types the client said it understands, as it named them.
    ///
    /// Written once, at `initialize`, and read on every colouring request. `None` when the client
    /// said nothing about semantic tokens at all, which is a different answer from an empty list --
    /// see `answers::tokens::offered`.
    pub(crate) understood_tokens: std::sync::OnceLock<Vec<SemanticTokenType>>,
    /// Whether the client draws progress bars, and whether it ignored being asked for one. Read at
    /// `initialize`; see [`Progress::start`] for why a bar is asked for the way it is.
    pub(crate) progress: Progress,
    /// Whether the client will re-ask for code lenses when told to.
    ///
    /// The one capability this server answers with *silence* before the index is ready -- a lens is
    /// a count of references and there is nothing honest to count yet -- and a client asks for
    /// lenses when a file opens and then not again. So without this the lenses of every file open
    /// at start-up never appear, and typing a character in one is what summons them, which reads
    /// as a cache that will not invalidate.
    pub(crate) supports_lens_refresh: AtomicBool,

    /// Whether the client asked for the `LocationLink` form of a navigation answer.
    ///
    /// A `Location` is a file and a range; a `LocationLink` adds the span the answer was *asked
    /// from*, which is what lets an editor underline the word under the cursor. The protocol makes
    /// it opt-in -- `textDocument.definition.linkSupport` and its three neighbours -- and a client
    /// that did not ask cannot deserialise one, so goto reads as a server that answers nothing.
    /// This server sent links unconditionally until a probe declaring `linkSupport: false` got
    /// them back and had no field to read.
    ///
    /// One flag for the four requests. The protocol has a bit apiece and no client in existence
    /// sets them apart, so a single answer is the honest reading of what the client said -- taken
    /// as *any of them asked*, since sending a `Location` to a client that wanted a link loses a
    /// nicety while the other direction loses the answer.
    pub(crate) supports_definition_links: AtomicBool,

    /// Whether the index is built, answered without touching the index.
    ///
    /// A second copy of what the skeleton's own state says, and the duplication is the point:
    /// reading the skeleton means taking the workspace lock, and an indexing pass holds that lock in
    /// batches for as long as indexing takes. So the gate meant to *refuse* a request during
    /// indexing could not be reached without first queueing behind the very thing it was refusing
    /// on behalf of -- measured at 3.8 s for a hover on a 30 000-file project, with every request
    /// behind it waiting the same. An editor that answers nothing for four seconds and then
    /// recovers is indistinguishable, from the outside, from one that has died.
    pub(crate) indexed: AtomicBool,
    /// Whether a write unwound and left the index half-updated.
    ///
    /// Tokio's `RwLock` does not poison, so this is the poisoning -- set by [`PoisoningWrite`]'s
    /// destructor and read by [`Self::require_ready`]. An `Arc` rather than a plain field because
    /// the guard outlives the borrow of `self` that produced it.
    ///
    /// Its own flag rather than reusing [`Self::failed`]: that one says a *pass* did not finish,
    /// which leaves an index that is incomplete but consistent, and the honest sentences differ.
    pub(crate) poisoned: Arc<AtomicBool>,
    /// Whether an indexing pass fell over instead of finishing.
    ///
    /// A pass runs on its own task now, and a task that panics is contained by the runtime rather
    /// than by the loop's containment, which covers a request's own task -- so the process survives and the index simply never
    /// arrives. Left at that, every request afterwards would be refused with "still indexing" for
    /// the rest of the session, which is a lie that looks exactly like a slow project. The refusal
    /// says which of the two it is instead.
    pub(crate) failed: AtomicBool,
    /// Held for the length of an indexing pass, so a second one queues rather than interleaves.
    ///
    /// Only reachable now that the message loop keeps turning: a `didChangeWatchedFiles` reporting a
    /// changed `composer.json` starts a pass that resets the index, and before this it could not
    /// arrive until the first pass had already finished.
    pub(crate) indexing: Mutex<()>,
    /// How many indexing passes have ended, however they ended.
    ///
    /// Startup used to finish inside the `initialized` notification, so the protocol's own ordering
    /// said when the index existed and the flow tests needed nothing else. It does not any more, and
    /// a test that asks a question before the answer exists tests the refusal rather than the
    /// feature. A count rather than a flag because a reconfigured project indexes again, and "it was
    /// ready once" is not the question a second pass raises.
    passes: watch::Sender<u64>,
    /// Which project-wide check is current. A pass reads it between files and stops when a newer
    /// one has been asked for, so a save during a pass costs one file rather than two passes.
    pub(crate) project_pass: AtomicU64,
    project_passes: watch::Sender<u64>,
    /// The closed files a project-wide check last reported problems in.
    ///
    /// A client keeps what it was last sent, so a file whose problems were fixed has to be told
    /// "none" -- and only that file: a pass publishing "none" for thirty thousand files that never
    /// had any would be thirty thousand notifications to say nothing.
    project_findings: std::sync::Mutex<Reported<PathBuf>>,
}

impl Session {
    pub(crate) fn over(client: Client, workspace: Workspace) -> Self {
        Self {
            client,
            workspace: Arc::new(RwLock::new(workspace)),
            progress: Progress::new(),
            supports_lens_refresh: AtomicBool::new(false),
            supports_definition_links: AtomicBool::new(false),
            supports_file_watching: AtomicBool::new(false),
            supports_rooted_watch: AtomicBool::new(false),
            waiting_for_composer: AtomicBool::new(false),
            reconfigured_for: std::sync::Mutex::new(None),
            supports_type_hierarchy: AtomicBool::new(false),
            supports_snippets: AtomicBool::new(false),
            understood_tokens: std::sync::OnceLock::new(),
            indexed: AtomicBool::new(false),
            poisoned: Arc::new(AtomicBool::new(false)),
            failed: AtomicBool::new(false),
            indexing: Mutex::new(()),
            passes: watch::Sender::new(0),
            project_pass: AtomicU64::new(0),
            project_passes: watch::Sender::new(0),
            project_findings: std::sync::Mutex::new(Reported::default()),
        }
    }

    /// A reader that sees every indexing pass that ends after this call.
    ///
    /// Nothing in the protocol asks this, which is why it is only compiled for the tests that do.
    #[cfg(test)]
    pub(crate) fn passes(&self) -> watch::Receiver<u64> {
        self.passes.subscribe()
    }

    /// Records that a pass is over, so anything waiting on one stops waiting.
    ///
    /// Called on the failure path too. Whoever is waiting is waiting for the pass to be *over*, and
    /// a pass that fell over is over.
    #[cfg(test)]
    pub(crate) fn project_passes(&self) -> watch::Receiver<u64> {
        self.project_passes.subscribe()
    }

    /// Every project-wide check says so when it ends, including one that had nothing to do, so a
    /// test can wait for it without knowing whether it ran.
    pub(crate) fn finished_a_project_pass(&self) {
        self.project_passes.send_modify(|passes| *passes += 1);
    }

    /// Records whether a closed file was reported with problems, and says whether its findings are
    /// worth sending: when it has some, or when the client was last told otherwise.
    pub(crate) fn note_project_findings(&self, path: &Path, any: bool) -> bool {
        let mut findings = self
            .project_findings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        findings.note(path, any)
    }

    pub(crate) fn finished_a_pass(&self) {
        self.passes.send_modify(|passes| *passes += 1);
    }

    /// Refuses a cross-file request while the project is not indexed.
    ///
    /// An honest "not yet" rather than a partial answer. Answering from a half-filled catalog is
    /// what makes features look like they switch themselves on one symbol at a time, and a user
    /// cannot tell that apart from a bug.
    /// The workspace, to change. Every write goes through this rather than through the lock, so
    /// that an unwound write is one the next reader is told about -- see [`PoisoningWrite`].
    pub(crate) async fn workspace_write(&self) -> PoisoningWrite<'_, Workspace> {
        PoisoningWrite::over(self.workspace.write().await, Arc::clone(&self.poisoned))
    }

    pub(crate) async fn require_ready(&self) -> Result<()> {
        // Before the ready check, and that order is the point: a poisoned index is *built*, so
        // `indexed` is true and every request would be answered from a state no file on disk
        // describes. See `PoisoningWrite` for why this refuses rather than rebuilding.
        if self.poisoned.load(Ordering::Relaxed) {
            return Err(Error {
                code: lspkit_runtime::protocol::jsonrpc::ErrorCode::ServerError(-32801),
                message: "phrost stopped part-way through changing its index, so what it holds \
                          now describes no state of your project -- the language server log says \
                          what broke, and restarting the server rebuilds it"
                    .into(),
                data: None,
            });
        }
        if self.indexed.load(Ordering::Relaxed) {
            return Ok(());
        }

        Err(Error {
            code: lspkit_runtime::protocol::jsonrpc::ErrorCode::ServerError(-32801),
            message: if self.failed.load(Ordering::Relaxed) {
                // A different sentence for a different state, on the same code. "Still indexing" is
                // an invitation to wait, and there is nothing to wait for.
                "phrost could not index this project -- the language server log says what broke, \
                 and restarting the server tries again"
                    .into()
            } else {
                "phrost is still indexing this project".into()
            },
            data: None,
        })
    }
}
