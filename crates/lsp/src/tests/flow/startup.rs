//! What startup owes a client that is slow, silent, or already showing a file.
//!
//! The two failures here have the same shape and the same cost: the server looks like it is working
//! and is not. One waits forever on an answer no client is obliged to send; the other publishes
//! diagnostics about an index the file it is describing had already fallen out of.

use expect_test::expect;

use crate::tests::Fixture;

/// One file that declares what it uses, so a missing ingest is visible as its own classes going
/// undefined rather than as something subtler.
const SELF_CONTAINED: &str = r#"
//- /src/Both.php
<?php
namespace App;

class Helper
{
    public function run(): void {}
}

class Caller
{
    public function go(Helper $helper): void
    {
        $helper->run();
    }
}
"#;

/// A buffer opened before the index exists is described from the buffer, not from the disk.
///
/// The order an editor actually uses: it restores its open tabs the moment `initialized` goes out,
/// so `didOpen` and bootstrap overlap on every real start. Bootstrap then reaches that file three
/// times the wrong way -- loading a cache adopts a skeleton the file is not in, the batch loop reads
/// it from *disk* and so misses everything unsaved, and forgetting the undiscovered can drop it
/// outright. The symptom is a class the file declares reading as undefined in the file that
/// declares it, corrected by no later publish and cured only by typing a character.
#[tokio::test]
async fn a_buffer_opened_while_indexing_survives_the_index_being_rebuilt_under_it() {
    let mut server = Fixture::new(SELF_CONTAINED);
    server.initialize(false, true).await;

    // A declaration that exists only in the editor, which is the ordinary state of a file somebody
    // is working in.
    let unsaved = format!(
        "{}\nclass Extra {{}}\n\nclass Later\n{{\n    public function go(Extra $extra): void {{}}\n}}\n",
        server.text("src/Both.php").trim_end()
    );
    server
        .notify(
            "textDocument/didOpen",
            serde_json::json!({ "textDocument": {
                "uri": server.uri("src/Both.php"), "languageId": "php", "version": 1,
                "text": unsaved,
            }}),
        )
        .await;

    server.initialized().await;

    expect![[r#"
        Both.php: none
    "#]]
    .assert_eq(&format!("{}\n", server.diagnostics_published().join("\n")));
}

/// The same, for a buffer that arrives *inside* the pass rather than before it.
///
/// A state that could not exist until indexing left the handler: the loop was stopped, so a
/// `didOpen` sent during startup was not read until startup was over. Now it lands in the middle,
/// and every window it can land in is covered by one thing -- the reingest that closes bootstrap
/// shares a critical section with the readiness flag, so no buffer can arrive between them.
///
/// The window here is the worst of them: after the cache has been adopted and before the batch loop,
/// which re-reads the file from *disk* and so discards everything the editor has not saved.
#[tokio::test(start_paused = true)]
async fn a_buffer_opened_inside_a_running_pass_is_still_described_from_the_buffer() {
    let mut server = Fixture::with_silent_client(SELF_CONTAINED);
    // Wants a progress bar from a client that answers nothing, so the pass parks between discovery
    // and the first indexed file and stays there for as long as this test has anything to do.
    server.initialize(true, false).await;

    let passes = server.watch_passes();
    server.notify("initialized", serde_json::json!({})).await;

    let unsaved = format!(
        "{}\nclass Extra {{}}\n\nclass Later\n{{\n    public function go(Extra $extra): void {{}}\n}}\n",
        server.text("src/Both.php").trim_end()
    );
    server
        .notify(
            "textDocument/didOpen",
            serde_json::json!({ "textDocument": {
                "uri": server.uri("src/Both.php"), "languageId": "php", "version": 1,
                "text": unsaved,
            }}),
        )
        .await;

    server.pass_ends(passes).await;

    expect![[r#"
        Both.php: none
    "#]]
    .assert_eq(&format!("{}\n", server.diagnostics_published().join("\n")));
}

/// A client that never answers a server-to-client request still gets an index.
///
/// Registering watchers, offering the class hierarchy and creating a progress token are all
/// requests, and all three were awaited without a bound. A client that ignores them -- which the
/// specification permits -- therefore stopped the server before it indexed anything, and every
/// request afterwards was answered with "still indexing" for as long as the session lasted.
#[tokio::test(start_paused = true)]
async fn a_client_that_answers_nothing_does_not_stop_the_index() {
    let mut server = Fixture::with_silent_client(SELF_CONTAINED);
    server.initialize(true, true).await;
    server.initialized().await;

    assert_eq!(
        server.logged_indexing().as_deref(),
        Some(
            "phrost indexed 1 of 1 discovered files (0 reused from cache, 0 of them without \
             reading), framework none detected"
        ),
    );
    expect![[r#"
        could not watch the project for changes made outside the editor -- the client did not answer within 5 seconds
        could not show a progress bar for Indexing -- the client did not answer within 5 seconds
        could not refresh the reference counts -- the client did not answer within 5 seconds
    "#]]
    .assert_eq(&format!("{}\n", server.logged_warnings().join("\n")));
}
