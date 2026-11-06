//! What the session does after a handler falls over.
//!
//! The thing worth proving is not that the panic happened -- it is that the server was still there
//! afterwards. Measured before any of this existed: a `panic!` in a handler unwound out of the
//! transport and out of `main`, the process exited 101, and the request that caused it was never
//! answered at all. So the test drives a real service -- lspkit's, which runs each request on a
//! task of its own and answers its panic as an error -- and asks the same question either side of
//! the panic.
//!
//! The service is built here rather than through `Fixture` for two reasons: this is the only test
//! that needs a handler which panics, and `Fixture` deliberately calls the bare service, so
//! everything else in the suite still dies loudly on a bug.

use std::sync::{Arc, Mutex};

use expect_test::expect;
use futures_util::{SinkExt as _, StreamExt as _};
use lspkit_runtime::panics;
use lspkit_runtime::protocol::{
    ClientSocket, LspService,
    jsonrpc::{Request, Response, Result},
};
use phrost_testkit::MaterializedFixture;
use serde_json::{Value, json};

use crate::{Backend, NAME, workspace::Workspace};

/// One class, so a `textDocument/definition` either side of the panic has something to find.
const ONE_CLASS: &str = r#"
//- /src/Repo.php
<?php
namespace App;

class Base
{
    public function run(): void {}
}

class Repo extends Ba@parent@se {}
"#;

impl Backend {
    /// A handler that always falls over. Test-only, and the only reachable panic in this crate.
    ///
    /// Registered as a custom method rather than by breaking a real one, so the panic is genuinely
    /// raised inside the router and the future the transport would have polled -- which is the
    /// path being tested -- while every real handler stays as it is.
    async fn panic_on_purpose(&self) -> Result<()> {
        panic!("a handler fell over");
    }

    /// A handler that falls over *while holding the write guard*, halfway through a change.
    ///
    /// The other half of the protocol, and the half containing a panic cannot make safe: tokio's `RwLock`
    /// does not poison, so without `PoisoningWrite` the guard is released and every request
    /// afterwards is answered from an index that describes no state of the project. Dropping the
    /// file first rather than panicking straight away, because a guard taken and released
    /// unchanged would prove nothing -- what is under test is a *half-updated* index.
    async fn panic_mid_write(&self) -> Result<()> {
        let mut workspace = self.workspace_write().await;
        // Every file dropped, which is what a re-ingest does before putting them back -- and the
        // moment a real pass is at its most half-finished.
        workspace.forget_undiscovered(&[]);
        panic!("a write fell over halfway");
    }
}

/// A server as lspkit serves it -- every request on a task of its own, its panic answered as an
/// error -- and a task draining its client socket.
struct Guarded {
    service: LspService<Backend>,
    project: MaterializedFixture,
    seen: Arc<Mutex<Vec<Request>>>,
    next_id: i64,
    /// The end of the indexing pass and of the project check, subscribed before either starts.
    passes: tokio::sync::watch::Receiver<u64>,
    project_passes: tokio::sync::watch::Receiver<u64>,
}

impl Guarded {
    fn new(fixture: &str) -> Self {
        let workspace = std::sync::Mutex::new(Some(Workspace::default()));
        let (service, socket) = LspService::build(move |client| {
            Backend::over(
                client,
                workspace
                    .lock()
                    .expect("built once")
                    .take()
                    .expect("built once"),
            )
        })
        .custom_method("test/panic", Backend::panic_on_purpose)
        .custom_method("test/panic-mid-write", Backend::panic_mid_write)
        .finish();

        // The name a contained panic is reported under, as `serve_stdio` installs it.
        panics::install(NAME);
        let passes = service.inner().passes();
        let project_passes = service.inner().project_passes();
        let seen = Arc::new(Mutex::new(Vec::new()));
        tokio::spawn(drain(socket, Arc::clone(&seen)));

        Self {
            service,
            project: MaterializedFixture::parse(fixture),
            seen,
            next_id: 0,
            passes,
            project_passes,
        }
    }

    async fn request(&mut self, method: &'static str, params: Value) -> Response {
        self.next_id += 1;
        let request = Request::build(method)
            .id(self.next_id)
            .params(params)
            .finish();

        let response = self
            .service
            .call(request)
            .await
            .expect("a request gets a response");

        tokio::task::yield_now().await;
        response
    }

    async fn notify(&mut self, method: &'static str, params: Value) {
        let request = Request::build(method).params(params).finish();
        let response = self.service.call(request).await;
        assert!(response.is_none(), "a notification has no response");
        tokio::task::yield_now().await;
    }

    /// Initializes and finishes bootstrap, the way a client would.
    async fn start(fixture: &str) -> Self {
        let mut server = Self::new(fixture);
        let root = server.project.root().to_path_buf();
        server
            .request(
                "initialize",
                json!({
                    "capabilities": {},
                    "workspaceFolders": [{
                        "uri": format!("file://{}", root.display()),
                        "name": "fixture",
                    }],
                }),
            )
            .await;
        server.notify("initialized", json!({})).await;
        // Both passes, so the log this test snapshots is complete before the panic is provoked:
        // the project check's line landing after the panic's is a race the run's timing decides.
        server
            .passes
            .changed()
            .await
            .expect("the server outlives startup");
        server
            .project_passes
            .changed()
            .await
            .expect("the server outlives startup");
        server.open("src/Repo.php").await;
        server
    }

    async fn open(&mut self, relative: &str) {
        let path = self.project.path(relative);
        let text = std::fs::read_to_string(&path).expect("fixture file is readable");
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

    /// Asks where the parent class is declared, and renders the answer whole.
    async fn definition(&mut self) -> String {
        let text = std::fs::read_to_string(self.project.path("src/Repo.php"))
            .expect("fixture file is readable");
        let index = phrost_model::LineIndex::new(&text);
        let offset = self.project.spec().markers().position("parent").offset;
        let at = crate::convert::position_of(&text, &index, offset);

        let response = self
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": format!("file://{}", self.project.path("src/Repo.php").display()) },
                    "position": { "line": at.line, "character": at.character },
                }),
            )
            .await;

        render(&response)
    }

    /// Every `window/logMessage` the server sent, as severity and text.
    fn logged(&self) -> Vec<String> {
        self.seen
            .lock()
            .expect("the drainer does not panic")
            .iter()
            .filter(|request| request.method() == "window/logMessage")
            .filter_map(|request| {
                let params = serde_json::to_value(request.params()?).ok()?;
                Some(redact(&format!(
                    "[{}] {}",
                    params["type"].as_i64().unwrap_or(-1),
                    params["message"].as_str()?
                )))
            })
            .collect()
    }
}

/// A response as one line: the error it carries, or the shape of the answer it carries.
fn render(response: &Response) -> String {
    if let Some(error) = response.error() {
        return redact(&format!(
            "error {}: {}",
            i64::from(error.code),
            error.message
        ));
    }
    let Some(result) = response.result() else {
        return "no result".to_owned();
    };
    match result {
        Value::Null => "null".to_owned(),
        // Either shape: a navigation answer comes back as a `LocationLink` where the client asked
        // for one and as a plain `Location` where it did not, and this fixture asks for neither.
        Value::Array(found) => found
            .iter()
            .map(|one| {
                let uri = one["targetUri"]
                    .as_str()
                    .or_else(|| one["uri"].as_str())
                    .unwrap_or("?");
                let file = uri.rsplit('/').next().unwrap_or(uri);
                let at = match one.get("targetSelectionRange") {
                    Some(range) => &range["start"],
                    None => &one["range"]["start"],
                };
                format!(
                    "{file} {}:{}",
                    at["line"].as_i64().unwrap_or(-1),
                    at["character"].as_i64().unwrap_or(-1)
                )
            })
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
}

/// Replaces the line and column of this file in a panic location, which move whenever it is edited.
///
/// The path is kept: that a caught panic can still say which file raised it is the point of the
/// hook, and a message that redacted the whole location would pass with the hook uninstalled.
fn redact(message: &str) -> String {
    const FILE: &str = "panics.rs:";

    let Some(start) = message.find(FILE) else {
        return message.to_owned();
    };
    let after = start + FILE.len();
    let rest = message[after..].trim_start_matches(|c: char| c.is_ascii_digit() || c == ':');

    format!("{}{FILE}L:C{rest}", &message[..start])
}

/// A panicking request answers with an error, says so in the log, and costs the session nothing.
#[tokio::test(flavor = "current_thread")]
async fn a_handler_that_panics_refuses_that_request_and_keeps_serving() {
    let mut server = Guarded::start(ONE_CLASS).await;

    let before = server.definition().await;
    let panicked = render(&server.request("test/panic", json!(null)).await);
    let after = server.definition().await;

    expect![[r#"
        before:  Repo.php 3:6
        panic:   error -32603: phrost failed while answering test/panic (request 3): a handler fell over (at crates/lsp/src/tests/flow/panics.rs:L:C)
        after:   Repo.php 3:6
    "#]]
    .assert_eq(&format!(
        "before:  {before}\npanic:   {panicked}\nafter:   {after}\n"
    ));

    expect![[r#"
        [2] this client cannot watch files, so changes made outside the editor (a branch switch, a composer install) will not be picked up
        [3] phrost indexed 1 of 1 discovered files (0 reused from cache, 0 of them without reading), framework none detected
        [3] phrost checked 1 of 1 files: 0 problems in 0 files
        [1] phrost failed while answering test/panic (request 3): a handler fell over (at crates/lsp/src/tests/flow/panics.rs:L:C)
    "#]]
    .assert_eq(&format!("{}\n", server.logged().join("\n")));
}

/// A write that falls over leaves an index nothing describes, and the next request is told.
///
/// The read case above keeps serving because a read changes nothing. This one cannot: the handler
/// has already dropped a file when it panics, so the index is neither what it was nor what the
/// project says. Answering from it would be the one outcome §1.2 forbids -- a plausible answer from
/// a state no file on disk has -- and it would keep being wrong for the rest of the session, which
/// is why the refusal names a restart the way `Session::failed` does.
///
/// The `before` line is the control: the same request answers correctly until the write unwinds.
#[tokio::test]
async fn a_write_that_panics_poisons_the_index_instead_of_answering_from_it() {
    let mut server = Guarded::start(ONE_CLASS).await;

    let before = server.definition().await;
    let panicked = render(&server.request("test/panic-mid-write", json!(null)).await);
    let after = server.definition().await;

    expect![[r#"
        before:  Repo.php 3:6
        panic:   error -32603: phrost failed while answering test/panic-mid-write (request 3): a write fell over halfway (at crates/lsp/src/tests/flow/panics.rs:L:C)
        after:   error -32801: phrost stopped part-way through changing its index, so what it holds now describes no state of your project -- the language server log says what broke, and restarting the server rebuilds it
    "#]].assert_eq(&format!(
        "before:  {before}\npanic:   {panicked}\nafter:   {after}\n"
    ));
}

/// Reads server-to-client traffic until the server stops, answering anything that expects a reply.
///
/// This test builds its service with methods outside the protocol, which `lspkit_flow::Flow` does
/// not, so it keeps a client of its own.
async fn drain(socket: ClientSocket, seen: Arc<Mutex<Vec<Request>>>) {
    let (mut requests, mut responses) = socket.split();

    while let Some(request) = requests.next().await {
        let id = request.id().cloned();
        seen.lock()
            .expect("no other panic holds the lock")
            .push(request);

        if let Some(id) = id {
            let response = Response::from_parts(id, Ok(Value::Null));
            if responses.send(response).await.is_err() {
                break;
            }
        }
    }
}
