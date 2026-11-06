//! Flow tests: the real server, driven in process.
//!
//! These cover the one class of bug nothing below can see. Every test underneath this layer asks a
//! question of a data structure; these ask it of a *server*, in an order, with state carried between
//! the steps. Version guards, readiness, and what an edit does to the next request only exist here —
//! and they are exactly what made the previous attempt feel unpredictable: "it works sometimes" is
//! almost always a sequencing bug, and almost never a wrong answer.
//!
//! The service is driven directly rather than over a pipe, so a test is deterministic. Indexing
//! runs on its own task, so `initialized` returns long before the index exists -- which is the whole
//! point of it, and would make every test below a test of the refusal instead of the feature. So
//! [`Fixture::initialized`] waits for the server's own end-of-pass signal: an edge, taken before the
//! notification is sent, so there is no sleeping and no polling either way.

mod a_composer_run_is_waited_out;
mod a_framework_configuration_file_is_watched;
mod a_session_gives_its_memory_back;
mod an_open_buffer_follows_the_disk;
mod flow;
mod watched;

use std::path::Path;

use lspkit_flow::{Client, Flow};
use lspkit_runtime::protocol::jsonrpc::Response;
use phrost_model::LineIndex;
use phrost_testkit::MaterializedFixture;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{Backend, convert, workspace::Workspace};

/// `FileChangeType` values, which the protocol sends as plain integers.
const FILE_CREATED: i32 = 1;
const FILE_CHANGED: i32 = 2;
const FILE_DELETED: i32 = 3;

/// Receivers for the end of the next indexing pass and of the project check that follows it.
pub(crate) struct Passes {
    index: watch::Receiver<u64>,
    project: watch::Receiver<u64>,
}

/// A server driven through [`Flow`], and the project it was pointed at.
///
/// What a project is, how the server is built over it and when its startup is over are phrost's;
/// the in-process client and everything that asks what it was sent are `lspkit-flow`'s.
struct Fixture {
    flow: Flow<Backend>,
    project: MaterializedFixture,
}

impl Fixture {
    /// A server over `workspace`, with a client that answers or records.
    fn serving(workspace: Workspace, client: Client) -> Flow<Backend> {
        Flow::new(move |client| Backend::over(client, workspace), client)
    }

    /// Materializes a fixture on disk and builds a server over it.
    ///
    /// On disk because bootstrap discovers from the filesystem, which is the thing being tested.
    fn new(fixture: &str) -> Self {
        Self::over(fixture, Workspace::default())
    }

    /// The same, over a workspace built by the test -- for the states that are about setup rather
    /// than about protocol behaviour.
    pub(crate) fn over(fixture: &str, workspace: Workspace) -> Self {
        Self {
            flow: Self::serving(workspace, Client::Answering),
            project: MaterializedFixture::parse(fixture),
        }
    }

    /// The same, over a client that records server-to-client traffic and answers none of it.
    ///
    /// A client is not obliged to answer a request it did not ask for, and one that does not is what
    /// found the startup hang. The answering client is what kept the whole class of bug invisible
    /// from in here.
    ///
    /// It is also the only way to hold a pass still. Under a paused clock the runtime advances time
    /// only when nothing is runnable, so a pass parked on the five-second wait for a reply that will
    /// never come stays parked for exactly as long as the test has something to do -- which is what
    /// makes "a request arriving mid-index" a fixed moment rather than a race.
    pub(crate) fn with_silent_client(fixture: &str) -> Self {
        Self {
            flow: Self::serving(Workspace::default(), Client::Silent),
            project: MaterializedFixture::parse(fixture),
        }
    }

    /// Replaces the server with a fresh one over the same directory.
    ///
    /// What an editor restart is, and the only way to test anything that is supposed to carry across
    /// one: the project on disk stays, everything in memory goes.
    pub(crate) fn restart(&mut self, workspace: Workspace) {
        self.flow = Self::serving(workspace, Client::Answering);
    }

    /// The line bootstrap logs when it finishes, which says how much of the index it had to build.
    fn logged_indexing(&self) -> Option<String> {
        self.flow
            .params("window/logMessage")
            .into_iter()
            .filter_map(|params| {
                let message = params["message"].as_str()?;
                message
                    .starts_with("phrost indexed")
                    .then(|| message.to_owned())
            })
            .next()
    }

    fn uri(&self, relative: &str) -> String {
        format!("file://{}", self.project.path(relative).display())
    }

    fn text(&self, relative: &str) -> String {
        std::fs::read_to_string(self.project.path(relative)).expect("fixture file is readable")
    }

    /// The LSP position of a fixture marker.
    pub(crate) fn position(&self, relative: &str, marker: &str) -> Value {
        let text = self.text(relative);
        let index = LineIndex::new(&text);
        let offset = self.project.spec().markers().position(marker).offset;
        let position = convert::position_of(&text, &index, offset);

        json!({ "line": position.line, "character": position.character })
    }

    async fn request(&mut self, method: &'static str, params: Value) -> Response {
        self.flow.request(method, params).await
    }

    /// Issues a request and hands back its id and its unfinished future.
    ///
    /// For the one test that has to do something *while* a request is in flight.
    async fn start(
        &mut self,
        method: &'static str,
        params: Value,
    ) -> (i64, futures_util::future::BoxFuture<'static, Response>) {
        self.flow.start(method, params).await
    }

    async fn notify(&mut self, method: &'static str, params: Value) {
        self.flow.notify(method, params).await;
    }

    /// Lets a debounced publish fire, on a paused clock.
    async fn wait_out_the_quiet(&self) {
        self.flow.after(std::time::Duration::from_millis(400)).await;
    }

    async fn settle(&self) {
        self.flow.settle().await;
    }

    /// Every method the server sent to the client, in order.
    fn methods_sent(&self) -> Vec<String> {
        self.flow.methods()
    }

    /// Every `$/progress` notification seen so far, by kind.
    fn progress_kinds(&self) -> Vec<String> {
        self.flow
            .params("$/progress")
            .into_iter()
            .filter_map(|params| Some(params["value"]["kind"].as_str()?.to_owned()))
            .collect()
    }

    /// Every set of diagnostics the server published, newest last, rendered for a snapshot.
    fn diagnostics_published(&self) -> Vec<String> {
        self.flow
            .params("textDocument/publishDiagnostics")
            .into_iter()
            .filter_map(|params| {
                let uri = params["uri"].as_str()?;
                let file = uri.rsplit('/').next()?.to_owned();
                let items = params["diagnostics"].as_array()?;
                if items.is_empty() {
                    return Some(format!("{file}: none"));
                }
                Some(
                    items
                        .iter()
                        .map(|item| {
                            format!(
                                "{file}:{}: {} {}",
                                item["range"]["start"]["line"].as_i64().unwrap_or(-1) + 1,
                                item["code"].as_str().unwrap_or("?"),
                                item["message"].as_str().unwrap_or("?")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            })
            .collect()
    }

    fn logged_at(&self, severity: i64) -> Vec<String> {
        self.flow.logged(severity)
    }

    /// Every warning the server logged, in order. `MessageType::WARNING` is 2.
    fn logged_warnings(&self) -> Vec<String> {
        self.flow.logged(2)
    }

    /// Every error the server logged, in order. `MessageType::ERROR` is 1.
    pub(crate) fn logged_errors(&self) -> Vec<String> {
        self.flow.logged(1)
    }

    /// The first warning the server logged, if any.
    fn logged_warning(&self) -> Option<String> {
        self.logged_warnings().into_iter().next()
    }

    // --- lifecycle, as a client would drive it ---

    async fn initialize(&mut self, work_done_progress: bool, watch_files: bool) -> Response {
        let root = self.project.root().to_path_buf();
        self.request(
            "initialize",
            json!({
                "capabilities": {
                    "window": { "workDoneProgress": work_done_progress },
                    "workspace": {
                        // Rooted watches too, which Zed, VS Code and Neovim all declare. A client
                        // that cannot root one has its own test.
                        "didChangeWatchedFiles": {
                            "dynamicRegistration": watch_files,
                            "relativePatternSupport": true,
                        },
                        // Declared unconditionally because a real editor does: a code lens is
                        // answered with silence until the index is ready, so the one thing that
                        // makes lenses appear at all is being told to ask again.
                        "codeLens": { "refreshSupport": true },
                    },
                    // Also unconditional, and for the same reason: every editor in use declares
                    // it, and a navigation answer comes back in the shape the client asked for.
                    // The client that asks for neither has its own test.
                    "textDocument": { "definition": { "linkSupport": true } },
                },
                "workspaceFolders": [{
                    "uri": format!("file://{}", root.display()),
                    "name": "fixture",
                }],
            }),
        )
        .await
    }

    /// Initializes with the settings an editor sends, and finishes startup.
    ///
    /// `initializationOptions` is the only place a client states these, and it is stated once -- so a
    /// test of them is a test of startup, and there is nothing to drive afterwards.
    pub(crate) async fn started_with_settings(fixture: &str, settings: Value) -> Self {
        let mut server = Self::new(fixture);
        let root = server.project.root().to_path_buf();
        server
            .request(
                "initialize",
                json!({
                    "capabilities": {},
                    "initializationOptions": settings,
                    "workspaceFolders": [{
                        "uri": format!("file://{}", root.display()),
                        "name": "fixture",
                    }],
                }),
            )
            .await;
        server.initialized().await;
        server
    }

    /// Finishes startup and waits for the index it starts.
    ///
    /// The handler does not: it spawns the pass and hands the message loop back, which is what a
    /// real client wants and what a test almost never does. Subscribing before the notification goes
    /// out is what makes the wait an edge rather than a race -- a pass that finishes first still
    /// leaves the receiver's version stale, so `changed` returns at once.
    async fn initialized(&mut self) {
        let mut passes = self.flow.server().passes();
        let mut project = self.flow.server().project_passes();
        self.notify("initialized", json!({})).await;
        passes.changed().await.expect("the server outlives startup");
        // And the check of the whole project that follows it, which publishes for closed files: a
        // test reading what was published needs both to be over, and one that switched the check
        // off still gets the signal, because a pass that had nothing to do says so too.
        project
            .changed()
            .await
            .expect("the server outlives startup");
        self.settle().await;
    }

    /// Watches for the end of an indexing pass a later step will start.
    ///
    /// Taken *before* that step, for the same reason [`Fixture::initialized`] takes one: the pass
    /// may be over by the time anything waits on it.
    pub(crate) fn watch_passes(&self) -> Passes {
        Passes {
            index: self.flow.server().passes(),
            project: self.flow.server().project_passes(),
        }
    }

    /// Watches for the end of a project check a later step will start, without an indexing pass
    /// before it -- a change on disk that reindexes one file.
    pub(crate) fn watch_project_passes(&self) -> watch::Receiver<u64> {
        self.flow.server().project_passes()
    }

    pub(crate) async fn project_pass_ends(&self, mut project: watch::Receiver<u64>) {
        project
            .changed()
            .await
            .expect("the server outlives the check");
        self.settle().await;
    }

    /// Waits out the pass a [`Fixture::watch_passes`] receiver was taken for.
    pub(crate) async fn pass_ends(&self, mut passes: Passes) {
        passes
            .index
            .changed()
            .await
            .expect("the server outlives the pass");
        passes
            .project
            .changed()
            .await
            .expect("the server outlives the pass");
        self.settle().await;
    }

    /// The server itself, for the tests that ask it something the protocol has no word for.
    pub(crate) fn backend(&self) -> &Backend {
        self.flow.server()
    }

    /// Initializes, bootstraps, and opens one document — the state most tests start from.
    async fn started(fixture: &str, open: &str) -> Self {
        let mut server = Self::new(fixture);
        server.initialize(false, true).await;
        server.initialized().await;
        server.open(open).await;
        server
    }

    async fn open(&mut self, relative: &str) {
        let uri = self.uri(relative);
        let text = self.text(relative);
        self.notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": uri, "languageId": "php", "version": 1, "text": text,
            }}),
        )
        .await;
    }

    /// Opens a file that is not part of the fixture, by its real path.
    ///
    /// What following a goto out of the project does: a stub lives in a cache directory and a
    /// dependency somewhere composer chose, and neither has a path relative to a fixture root.
    async fn open_absolute(&mut self, path: &std::path::Path) {
        let text = std::fs::read_to_string(path).expect("the file is readable");
        self.notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": format!("file://{}", path.display()),
                "languageId": "php",
                "version": 1,
                "text": text,
            }}),
        )
        .await;
    }

    async fn change(&mut self, relative: &str, version: i64, text: &str) {
        let uri = self.uri(relative);
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }],
            }),
        )
        .await;
    }

    async fn close(&mut self, relative: &str) {
        let uri = self.uri(relative);
        self.notify(
            "textDocument/didClose",
            json!({ "textDocument": { "uri": uri } }),
        )
        .await;
    }

    /// Writes files into the project and tells the server about them, as a client watcher would.
    async fn write_and_notify(&mut self, fixture: &str) {
        let written = self.project.write_more(fixture);
        let changes = written
            .iter()
            .map(|file| json!({ "uri": self.uri(file.path()), "type": FILE_CREATED }))
            .collect::<Vec<_>>();

        self.notify(
            "workspace/didChangeWatchedFiles",
            json!({ "changes": changes }),
        )
        .await;
    }

    /// Removes a file from disk and tells the server, as a client watcher would.
    async fn delete_and_notify(&mut self, relative: &str) {
        self.project.remove(relative);
        self.notify(
            "workspace/didChangeWatchedFiles",
            json!({ "changes": [{ "uri": self.uri(relative), "type": FILE_DELETED }] }),
        )
        .await;
    }

    /// Reports a change without touching the file, for the cases where that is the point.
    async fn notify_changed(&mut self, relative: &str) {
        self.notify_watched(relative, FILE_CHANGED).await;
    }

    /// The same for a path that appeared, which is what a client reports for a directory as well.
    async fn notify_created(&mut self, relative: &str) {
        self.notify_watched(relative, FILE_CREATED).await;
    }

    /// The same for a path that went away, without touching the disk.
    async fn notify_deleted(&mut self, relative: &str) {
        self.notify_watched(relative, FILE_DELETED).await;
    }

    async fn notify_watched(&mut self, relative: &str, kind: i32) {
        self.notify(
            "workspace/didChangeWatchedFiles",
            json!({ "changes": [{ "uri": self.uri(relative), "type": kind }] }),
        )
        .await;
    }

    /// What the server asked the client to watch.
    ///
    /// A rooted watch is rendered `<base>|<pattern>`, with the base relative to the project, so a
    /// test can tell it from a glob that only happens to end the same way.
    fn watcher_globs(&self) -> Vec<String> {
        let root = self.project.root().to_path_buf();
        self.flow
            .params("client/registerCapability")
            .into_iter()
            .flat_map(|params| {
                params["registrations"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            })
            .flat_map(|registration| {
                registration["registerOptions"]["watchers"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            })
            .filter_map(|watcher| {
                let pattern = &watcher["globPattern"];
                if let Some(glob) = pattern.as_str() {
                    return Some(glob.to_owned());
                }
                let base = pattern["baseUri"].as_str()?;
                let base = base.strip_prefix("file://").unwrap_or(base);
                let base = Path::new(base).strip_prefix(&root).ok()?;
                Some(format!(
                    "{}|{}",
                    base.display(),
                    pattern["pattern"].as_str()?
                ))
            })
            .collect()
    }

    async fn definition(&mut self, relative: &str, marker: &str) -> Response {
        let params = json!({
            "textDocument": { "uri": self.uri(relative) },
            "position": self.position(relative, marker),
        });
        self.request("textDocument/definition", params).await
    }

    async fn folding(&mut self, relative: &str) -> Response {
        let params = json!({ "textDocument": { "uri": self.uri(relative) } });
        self.request("textDocument/foldingRange", params).await
    }

    async fn semantic_tokens(&mut self, relative: &str) -> Response {
        let params = json!({ "textDocument": { "uri": self.uri(relative) } });
        self.request("textDocument/semanticTokens/full", params)
            .await
    }

    async fn semantic_tokens_in(&mut self, relative: &str, lines: (u32, u32)) -> Response {
        let params = json!({
            "textDocument": { "uri": self.uri(relative) },
            "range": {
                "start": { "line": lines.0, "character": 0 },
                "end": { "line": lines.1, "character": 0 },
            },
        });
        self.request("textDocument/semanticTokens/range", params)
            .await
    }

    async fn hover(&mut self, relative: &str, marker: &str) -> Response {
        let params = json!({
            "textDocument": { "uri": self.uri(relative) },
            "position": self.position(relative, marker),
        });
        self.request("textDocument/hover", params).await
    }
}

/// The relative path a definition response points at, or `None` when it found nothing.
fn definition_target(response: &Response, root: &Path) -> Option<String> {
    let result = serde_json::to_value(response.result()?).ok()?;
    let uri = result.get(0)?["targetUri"].as_str()?.to_owned();
    let path = uri.strip_prefix("file://")?;

    Some(
        Path::new(path)
            .strip_prefix(root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/"),
    )
}

/// The error message of a refused request.
fn error_message(response: &Response) -> Option<String> {
    Some(response.error()?.message.to_string())
}
