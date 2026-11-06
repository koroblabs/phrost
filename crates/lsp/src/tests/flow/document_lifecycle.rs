//! What an edit, a close, or an unknown document does to the very next request.
//!
//! No debounce and no save required: an edit is visible to the next request or the index is lying
//! about what is on screen. Ordering is enforced the same way -- an out-of-order version is ignored
//! and reported, never applied -- and a document the server never opened, or a position past the
//! end of one it has, answers nothing rather than erroring.

use expect_test::expect;

use crate::tests::flow::TWO_FILES;
use crate::tests::{Fixture, definition_target};

#[tokio::test]
async fn a_request_about_a_document_the_server_never_saw_answers_nothing() {
    // A scratch buffer, or a file outside the project. Normal for a client to ask about, and not an
    // error — returning one would put a red squiggle on something that is merely unknown.
    let mut server = Fixture::new(TWO_FILES);
    server.initialize(false, true).await;
    server.initialized().await;

    let response = server.definition("src/Repo.php", "parent").await;
    assert!(
        response.error().is_none(),
        "an unknown document is not an error"
    );
    assert!(definition_target(&response, server.project.root()).is_none());
}

#[tokio::test]
async fn an_edit_is_visible_to_the_very_next_request() {
    // No debounce to wait out, no save required. The failure this guards against is an index that
    // updates on save only, where the answer silently lags behind what is on screen.
    let mut server = Fixture::started(TWO_FILES, "src/Repo.php").await;
    server.open("src/Base.php").await;

    server
        .change(
            "src/Base.php",
            2,
            "<?php\nnamespace App;\n\nclass Renamed\n{\n}\n",
        )
        .await;

    let response = server.definition("src/Repo.php", "parent").await;
    assert!(
        definition_target(&response, server.project.root()).is_none(),
        "the old name must stop resolving once the declaration is gone"
    );
}

#[tokio::test]
async fn a_declaration_added_by_an_edit_resolves_immediately() {
    let mut server = Fixture::started(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo extends Ba@parent@se {}
"#,
        "src/Repo.php",
    )
    .await;

    // Nothing declares `App\Base` yet.
    let before = server.definition("src/Repo.php", "parent").await;
    assert!(definition_target(&before, server.project.root()).is_none());

    // Writing it into the open buffer is enough; no save, no rescan. The new class goes *after* the
    // reference so the marker's line and column are still the reference -- a position taken from the
    // text before an edit means nothing after it.
    server
        .change(
            "src/Repo.php",
            2,
            "<?php\nnamespace App;\n\nclass Repo extends Base {}\nclass Base {}\n",
        )
        .await;

    let after = server.definition("src/Repo.php", "parent").await;
    expect!["src/Repo.php"].assert_eq(
        &definition_target(&after, server.project.root()).expect("the new declaration resolves"),
    );
}

#[tokio::test]
async fn closing_a_document_keeps_what_it_declared() {
    // Closing drops the buffer and nothing else. The previous attempt re-read the file from disk
    // here, under a write lock, which is where "closing a file hangs the editor" came from — and
    // dropping the declaration instead would make goto stop working when a tab is closed.
    let mut server = Fixture::started(TWO_FILES, "src/Base.php").await;
    server.open("src/Repo.php").await;
    server.close("src/Base.php").await;

    let response = server.definition("src/Repo.php", "parent").await;
    expect!["src/Base.php"].assert_eq(
        &definition_target(&response, server.project.root())
            .expect("a closed file still declares its class"),
    );
}

#[tokio::test]
async fn a_position_past_the_end_of_a_document_is_answered_not_refused() {
    // Happens whenever the client is a revision ahead of the server. Failing the request would mean
    // features flickering off while someone types.
    let mut server = Fixture::started(TWO_FILES, "src/Repo.php").await;

    let params = serde_json::json!({
        "textDocument": { "uri": server.uri("src/Repo.php") },
        "position": { "line": 9999, "character": 9999 },
    });
    let response = server.request("textDocument/definition", params).await;

    assert!(
        response.error().is_none(),
        "a stale position is not an error"
    );
    assert!(definition_target(&response, server.project.root()).is_none());
}

#[tokio::test]
async fn an_edit_older_than_the_one_held_is_ignored() {
    // The protocol promises ordered notifications per document, so this should never happen. It is
    // worth guarding because the failure is invisible: the index would quietly go backwards while
    // the editor shows something else, and no later answer could tell.
    let mut server = Fixture::started(TWO_FILES, "src/Repo.php").await;
    server.open("src/Base.php").await;

    server
        .change(
            "src/Base.php",
            5,
            "<?php\nnamespace App;\n\nclass Renamed {}\n",
        )
        .await;
    // Version 2 arriving after version 5.
    server
        .change(
            "src/Base.php",
            2,
            "<?php\nnamespace App;\n\nclass Base {}\n",
        )
        .await;

    let response = server.definition("src/Repo.php", "parent").await;
    assert!(
        definition_target(&response, server.project.root()).is_none(),
        "the stale edit must not resurrect the old declaration"
    );
    assert!(
        server
            .logged_warning()
            .is_some_and(|message| message.contains("already held")),
        "an ignored edit has to be reported: {:?}",
        server.logged_warning()
    );
}
