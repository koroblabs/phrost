//! Walking a call graph through a real server: prepare, then incoming, then outgoing.
//!
//! The sequence is the point. A client sends three separate requests and the only thing carrying
//! state between them is the item, so an item that cannot be resolved back produces a tree that
//! expands to nothing -- and nothing below this layer can see that, because everything below is
//! handed the subject directly.
//!
//! Two claims this file exists to make:
//!
//! - **Outgoing calls work for a target in a closed file.** After the first step of the tree, every
//!   item is one: the callers of an open file's method are in files nobody opened. The body has to
//!   be produced by reading the file into a scratch buffer set, which is what the reference sweep
//!   already does, and only a test that expands a *returned* item proves it happens.
//! - **The search is cancellable.** Incoming calls is a project-wide sweep, which is the one request
//!   long enough for a client to change its mind about -- the same reason `cancel.rs` exists next
//!   door -- and it must not hold the workspace lock while it runs.

use std::time::Duration;

use expect_test::expect;
use lspkit_runtime::protocol::jsonrpc::Response;
use serde_json::{Value, json};

use crate::tests::{Fixture, error_message, flow::render_span};

/// One repository method called from two other files, one of which calls on to a third.
const CALLED_ACROSS_FILES: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function fi@declaration@nd(int $id): void {}
    public function save(int $id): void {}
}

//- /src/One.php
<?php
namespace App;

class One
{
    public function run(Repo $repo): void
    {
        $repo->find(1);
        $repo->find(2);
        $repo->save(3);
    }
}

//- /src/Two.php
<?php
namespace App;

class Two
{
    public function run(Repo $repo): void
    {
        $repo->find(3);
    }
}
"#;

#[tokio::test]
async fn a_tree_is_walked_from_a_declaration_to_its_callers_and_on_into_a_closed_file() {
    let mut server = Fixture::started(CALLED_ACROSS_FILES, "src/Repo.php").await;

    let prepared = server
        .request(
            "textDocument/prepareCallHierarchy",
            json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": server.position("src/Repo.php", "declaration"),
            }),
        )
        .await;
    let head = first_item(&prepared).expect("the cursor is on a method");

    let callers = server
        .request("callHierarchy/incomingCalls", json!({ "item": head }))
        .await;

    // The second step of the tree, on an item the client was *given* rather than one it prepared:
    // `App\One::run` is declared in a file nobody has open, and its body has to be read to answer.
    let caller = incoming_items(&callers)
        .into_iter()
        .find(|item| item["detail"] == json!("App\\One::run"))
        .expect("One::run calls find");
    let called = server
        .request("callHierarchy/outgoingCalls", json!({ "item": caller }))
        .await;

    expect![[r#"
        [
            "find App\\Repo::find [6] Repo.php 5:4-5:42 name 5:20-5:24",
        ]
    "#]]
    .assert_debug_eq(&render_items(&prepared));
    expect![[r#"
        [
            "App\\One::run [6] One.php 5:4-10:5 name 5:20-5:23 called at 7:15-7:19, 8:15-8:19",
            "App\\Two::run [6] Two.php 5:4-8:5 name 5:20-5:23 called at 7:15-7:19",
        ]
    "#]]
    .assert_debug_eq(&render_incoming(&callers));
    expect![[r#"
        [
            "App\\Repo::find [6] Repo.php 5:4-5:42 name 5:20-5:24 called at 7:15-7:19, 8:15-8:19",
            "App\\Repo::save [6] Repo.php 6:4-6:42 name 6:20-6:24 called at 9:15-9:19",
        ]
    "#]]
    .assert_debug_eq(&render_outgoing(&called));
}

#[tokio::test]
async fn a_cancelled_search_for_callers_is_refused_and_leaves_the_server_answering() {
    // The same shape `cancel.rs` proves for find-references, asked of the other project-wide
    // request: both go through one sweep, which takes the lock per candidate file and lets it go
    // before yielding, so an abort between two files leaves nothing held.
    let mut server = Fixture::started(&many_callers(20), "src/Repo.php").await;

    let prepared = server
        .request(
            "textDocument/prepareCallHierarchy",
            json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": server.position("src/Repo.php", "declaration"),
            }),
        )
        .await;
    let head = first_item(&prepared).expect("the cursor is on a method");

    let (id, pending) = server
        .start("callHierarchy/incomingCalls", json!({ "item": head }))
        .await;

    // Polled once first, so the handler is inside the sweep when the cancel arrives.
    let mut pending = pending;
    tokio::select! {
        biased;
        _ = &mut pending => panic!("the search finished before it could be cancelled"),
        () = tokio::task::yield_now() => {}
    }

    server.notify("$/cancelRequest", json!({ "id": id })).await;

    let response = pending.await;
    expect!["Canceled"]
        .assert_eq(&error_message(&response).expect("a cancelled request is refused"));

    let after = server.hover("src/Repo.php", "declaration").await;
    assert!(
        after.result().is_some(),
        "the server stopped answering after a cancel: {:?}",
        after.error()
    );
}

#[tokio::test]
async fn a_search_for_callers_does_not_block_anything_else() {
    let mut server = Fixture::started(&many_callers(20), "src/Repo.php").await;

    let prepared = server
        .request(
            "textDocument/prepareCallHierarchy",
            json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": server.position("src/Repo.php", "declaration"),
            }),
        )
        .await;
    let head = first_item(&prepared).expect("the cursor is on a method");

    let (_, pending) = server
        .start("callHierarchy/incomingCalls", json!({ "item": head }))
        .await;
    let mut pending = pending;
    tokio::select! {
        biased;
        _ = &mut pending => panic!("the search finished before anything could be asked beside it"),
        () = tokio::task::yield_now() => {}
    }

    // Under a timeout, so a regression fails instead of hanging.
    let beside = tokio::time::timeout(
        Duration::from_secs(5),
        server.hover("src/Repo.php", "declaration"),
    )
    .await
    .expect("a request beside a running search must not wait for it");
    assert!(
        beside.result().is_some(),
        "the request beside the search answered nothing: {:?}",
        beside.error()
    );

    let finished = tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .expect("the search finishes");
    let found = serde_json::to_value(finished.result().expect("the search answered"))
        .expect("a caller list is json");
    assert_eq!(
        found.as_array().map(Vec::len),
        Some(20),
        "expected twenty callers, got {found}"
    );
}

/// Enough files that a sweep spans several turns of the runtime rather than finishing in one.
fn many_callers(count: usize) -> String {
    let mut out = String::from(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function fi@declaration@nd(int $id): void {}
}
"#,
    );

    for n in 0..count {
        out.push_str(&format!(
            r#"
//- /src/Caller{n}.php
<?php
namespace App;

class Caller{n}
{{
    public function run(Repo $repo): void
    {{
        $repo->find({n});
    }}
}}
"#
        ));
    }
    out
}

/// The first item of a `prepareCallHierarchy` response, as a client would hand it back.
fn first_item(response: &Response) -> Option<Value> {
    let value = serde_json::to_value(response.result()?).ok()?;
    value.as_array()?.first().cloned()
}

/// The `from` item of each incoming call, as a client would hand one back to expand the tree.
fn incoming_items(response: &Response) -> Vec<Value> {
    let Some(result) = response.result() else {
        return Vec::new();
    };
    let value = serde_json::to_value(result).unwrap_or(Value::Null);
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|call| call["from"].clone())
        .collect()
}

fn render_items(response: &Response) -> Vec<String> {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            format!(
                "{} {}",
                item["name"].as_str().unwrap_or("?"),
                render_item(item)
            )
        })
        .collect()
}

fn render_incoming(response: &Response) -> Vec<String> {
    render_calls(response, "from")
}

fn render_outgoing(response: &Response) -> Vec<String> {
    render_calls(response, "to")
}

/// One line per call: the item it names, then the sites the call was written at.
///
/// The sites are rendered as bare spans on purpose. An incoming call's ranges are in the item's own
/// file and an outgoing call's are in the file the request was *about*, and printing a filename
/// beside them would state the wrong one for half the rows -- so the difference is asserted by the
/// numbers, against a fixture whose two files do not share a line layout.
fn render_calls(response: &Response, side: &str) -> Vec<String> {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|call| {
            let spans = call["fromRanges"]
                .as_array()
                .into_iter()
                .flatten()
                .map(render_span)
                .collect::<Vec<_>>();
            format!(
                "{} called at {}",
                render_item(&call[side]),
                spans.join(", ")
            )
        })
        .collect()
}

/// An item as everything a client shows or navigates by.
fn render_item(item: &Value) -> String {
    format!(
        "{} [{}] {} {} name {}",
        item["detail"].as_str().unwrap_or("?"),
        item["kind"].as_i64().unwrap_or(-1),
        file_of(item["uri"].as_str().unwrap_or("")),
        render_span(&item["range"]),
        render_span(&item["selectionRange"])
    )
}

fn file_of(uri: &str) -> &str {
    uri.rsplit('/').next().unwrap_or(uri)
}
