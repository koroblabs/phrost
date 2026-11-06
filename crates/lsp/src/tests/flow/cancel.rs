//! What happens to a request the client changes its mind about, and to everything asked beside it.
//!
//! A project-wide search is the one request here long enough for a client to cancel, and the one
//! that takes the workspace lock over and over. Which is the whole risk, and it is not theoretical:
//! the previous attempt at this server hung on find-references once a project had enough files,
//! because one long request held what every other request needed.
//!
//! So neither test here is about the error code -- the protocol library arranges that. They are
//! about the server still working: after a cancel, and *during* a search.

use std::time::Duration;

use expect_test::expect;

use crate::tests::{Fixture, error_message};

/// Enough files that a sweep spans several turns of the runtime rather than finishing in one.
fn many_files(count: usize) -> String {
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

/// The reference search, as the client sends it.
fn search(server: &Fixture) -> serde_json::Value {
    serde_json::json!({
        "textDocument": { "uri": server.uri("src/Repo.php") },
        "position": server.position("src/Repo.php", "declaration"),
        "context": { "includeDeclaration": true },
    })
}

#[tokio::test]
async fn a_cancelled_search_is_refused_and_leaves_the_server_answering() {
    let mut server = Fixture::started(&many_files(20), "src/Repo.php").await;

    let params = search(&server);
    let (id, pending) = server.start("textDocument/references", params).await;

    // Polled once first, so the handler is inside the sweep when the cancel arrives. Cancelling a
    // request that has not started proves only that the protocol library works.
    let mut pending = pending;
    tokio::select! {
        biased;
        _ = &mut pending => panic!("the search finished before it could be cancelled"),
        () = tokio::task::yield_now() => {}
    }

    server
        .notify("$/cancelRequest", serde_json::json!({ "id": id }))
        .await;

    let response = pending.await;
    expect!["Canceled"]
        .assert_eq(&error_message(&response).expect("a cancelled request is refused"));

    // The half that matters. A sweep takes the lock once per file and lets it go before yielding, so
    // an abort between two files leaves nothing held -- and the only way to say that out loud is to
    // ask for something afterwards.
    let after = server.hover("src/Repo.php", "declaration").await;
    assert!(
        after.result().is_some(),
        "the server stopped answering after a cancel: {:?}",
        after.error()
    );
}

#[tokio::test]
async fn a_search_in_flight_does_not_block_anything_else() {
    // The failure the previous attempt had: one long request holding what every other request
    // needs, and an editor that stops responding while a search runs.
    //
    // Under a timeout, so a regression fails instead of hanging. A test that hangs is the worst way
    // for a test to be wrong -- nobody reads a suite that never finishes.
    let mut server = Fixture::started(&many_files(20), "src/Repo.php").await;

    let params = search(&server);
    let (_, pending) = server.start("textDocument/references", params).await;
    let mut pending = pending;
    tokio::select! {
        biased;
        _ = &mut pending => panic!("the search finished before anything could be asked beside it"),
        () = tokio::task::yield_now() => {}
    }

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

    // And the search still finishes, rather than having been starved by the answer above.
    let finished = tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .expect("the search finishes");
    let found = serde_json::to_value(finished.result().expect("the search answered"))
        .expect("a location list is json");
    assert_eq!(
        found.as_array().map(Vec::len),
        Some(21),
        "expected the declaration and twenty callers, got {found}"
    );
}
