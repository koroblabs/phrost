//! Whether the server is ready to answer, and what it tells the client about getting there.
//!
//! A request before bootstrap finishes must be refused outright, never answered from a half-filled
//! catalog -- that is indistinguishable from a broken server one symbol at a time. Progress and the
//! missing-stub warning are the client-visible half of the same state machine, and the warm-restart
//! test belongs here too: it is bootstrap answering "already done" instead of "not yet".

use expect_test::expect;
use serde_json::json;

use crate::NAME;
use crate::tests::flow::TWO_FILES;
use crate::tests::{Fixture, definition_target, error_message};
use crate::workspace::Workspace;

#[tokio::test]
async fn a_cross_file_request_before_indexing_finishes_is_refused_not_guessed() {
    // The bug this rules out: answering from a half-filled catalog, so a feature appears to switch
    // itself on one symbol at a time. A user cannot tell that apart from a broken server.
    let mut server = Fixture::new(TWO_FILES);
    server.initialize(false, true).await;
    server.open("src/Repo.php").await;

    let response = server.definition("src/Repo.php", "parent").await;
    expect!["phrost is still indexing this project"]
        .assert_eq(&error_message(&response).expect("the request is refused"));
}

/// A request that arrives while the index is being built is refused now, not answered later.
///
/// The bug this rules out is the whole reason indexing left the handler. `initialized` ran it
/// inline; notifications are finished in order before the next message is read -- under
/// `tower-lsp-server` then, and under lspkit's own loop now, which keeps the ordering on purpose --
/// so nothing else advanced for as long as indexing took. Measured
/// on a 30 000-file project: a hover issued straight after `didOpen` came back after 5 465 ms and
/// was *answered*. That is the proof rather than the symptom -- had its future been polled at all,
/// the readiness gate would have refused it in microseconds. From outside, a server that says
/// nothing for five seconds and then recovers is one that has died.
///
/// A silent client and a paused clock hold the pass still: it is waiting out five seconds for
/// permission to draw an indexing bar, and virtual time only advances when nothing is runnable. So
/// the refusal below is the message loop turning, not a race won.
#[tokio::test(start_paused = true)]
async fn a_request_arriving_while_the_index_is_built_is_refused_rather_than_parked() {
    let mut server = Fixture::with_silent_client(TWO_FILES);
    // Wants a progress bar and cannot watch files, so the pass runs as far as the one client request
    // it will hang on and no further.
    server.initialize(true, false).await;
    server.open("src/Repo.php").await;

    let passes = server.watch_passes();
    server.notify("initialized", json!({})).await;

    let refused = server.definition("src/Repo.php", "parent").await;
    expect!["phrost is still indexing this project"]
        .assert_eq(&error_message(&refused).expect("the request is refused, not parked"));

    server.pass_ends(passes).await;
    let answered = server.definition("src/Repo.php", "parent").await;
    expect!["src/Base.php"].assert_eq(
        &definition_target(&answered, server.project.root()).expect("the parent is found"),
    );
}

/// An indexing pass that falls over says so, instead of claiming to be indexing for ever.
///
/// A pass runs on its own task, and the loop's containment does not reach it: that covers a
/// request's own task, and a panic in a spawned task goes into a `JoinHandle` nobody holds. So what the crash becomes here is
/// an index that never arrives -- every request afterwards refused with "still indexing", for the
/// rest of the session, with nothing said. A permanent lie is worse than the crash it replaced.
#[tokio::test]
async fn an_indexing_pass_that_falls_over_stops_claiming_to_be_indexing() {
    // What the report calls the server is installed by `serve_stdio`, which `serve` calls and
    // `Fixture` deliberately does not. Without it the log reads "the language server failed while
    // ...", which is not the line a person reads in phrost's log.
    lspkit_runtime::panics::install(NAME);
    let mut server = Fixture::new(TWO_FILES);
    server.initialize(false, true).await;
    server.open("src/Repo.php").await;

    let passes = server.watch_passes();
    server
        .backend()
        .indexing_pass(async { panic!("indexing fell over") })
        .await;
    server.pass_ends(passes).await;

    let response = server.definition("src/Repo.php", "parent").await;
    expect![
        "phrost could not index this project -- the language server log says what broke, and \
         restarting the server tries again"
    ]
    .assert_eq(&error_message(&response).expect("the request is refused"));

    // Without its location: installing the name installed the panic hook too, so the location is
    // there, and it is a line of this file that would move the snapshot with every edit above it.
    let logged = server
        .logged_errors()
        .iter()
        .map(|line| line.split(" (at ").next().unwrap_or(line).to_owned())
        .collect::<Vec<_>>();
    expect!["phrost failed while indexing the project: indexing fell over"]
        .assert_eq(&logged.join("\n"));
}

#[tokio::test]
async fn once_indexing_finishes_the_same_request_is_answered() {
    let mut server = Fixture::started(TWO_FILES, "src/Repo.php").await;

    let response = server.definition("src/Repo.php", "parent").await;
    expect!["src/Base.php"].assert_eq(
        &definition_target(&response, server.project.root()).expect("the parent is found"),
    );
}

#[tokio::test]
async fn progress_is_silent_unless_the_client_asked_for_it() {
    // Reporting progress against a token a client never agreed to is a protocol violation, and some
    // clients answer one by closing the connection.
    let mut server = Fixture::new(TWO_FILES);
    server.initialize(false, true).await;
    server.initialized().await;

    assert!(
        server.progress_kinds().is_empty(),
        "no progress should be sent, got {:?}",
        server.progress_kinds()
    );
}

#[tokio::test]
async fn a_missing_standard_library_is_reported_not_swallowed() {
    // Without stubs, every reference to `strlen` resolves to nothing -- which is indistinguishable
    // from a broken index unless the server says why.
    let mut server = Fixture::over(
        TWO_FILES,
        Workspace::without_stubs("no cache dir".to_owned()),
    );
    server.initialize(false, true).await;
    server.initialized().await;

    let warning = server.logged_warning().unwrap_or_default();
    assert!(
        warning.contains("no cache dir") && warning.contains("standard library"),
        "expected a stub warning, got {warning:?}"
    );
}

#[tokio::test]
async fn the_progress_token_is_created_before_it_is_reported_against() {
    // Zed silently drops `$/progress` for a token it was not asked to create, so this ordering is
    // the difference between a visible indexing bar and a server that looks hung.
    let mut server = Fixture::new(TWO_FILES);
    server.initialize(true, true).await;
    server.initialized().await;

    let methods = server.methods_sent();
    let create = methods
        .iter()
        .position(|method| method == "window/workDoneProgress/create");
    let first_progress = methods.iter().position(|method| method == "$/progress");
    assert!(
        matches!((create, first_progress), (Some(create), Some(progress)) if create < progress),
        "create must come first: {methods:?}"
    );
}

#[tokio::test]
async fn progress_begins_reports_and_ends_when_the_client_asked() {
    let mut server = Fixture::new(TWO_FILES);
    server.initialize(true, true).await;
    server.initialized().await;

    let kinds = server.progress_kinds();
    assert_eq!(
        kinds.first().map(String::as_str),
        Some("begin"),
        "{kinds:?}"
    );
    assert_eq!(kinds.last().map(String::as_str), Some("end"), "{kinds:?}");
    assert!(
        kinds.iter().any(|kind| kind == "report"),
        "a bar that never advances is worse than none: {kinds:?}"
    );
}

#[tokio::test]
async fn a_second_start_reuses_what_the_first_wrote() {
    // The whole point, and the one thing a unit test on the codec cannot show: a server started over
    // a project it has already indexed does not parse the files again -- and still answers the same.
    let cache = tempfile::tempdir().expect("a temporary directory");
    let stubs = phrost_stubs::StubSet::empty();

    let mut server = Fixture::over(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo {}

//- /src/Service.php
<?php
namespace App;

class Service
{
    public function run(): Repo
    {
        return new Repo();
    }
}
"#,
        Workspace::caching_in(stubs.clone(), cache.path()),
    );
    server.initialize(false, false).await;
    server.initialized().await;
    let cold = server.logged_indexing().unwrap_or_default();

    server.open("src/Service.php").await;
    let before = server
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": server.uri("src/Service.php") },
                "position": { "line": 7, "character": 20 },
            }),
        )
        .await;
    let cold_target = definition_target(&before, server.project.root()).unwrap_or_default();

    server.restart(Workspace::caching_in(stubs, cache.path()));
    server.initialize(false, false).await;
    server.initialized().await;
    let warm = server.logged_indexing().unwrap_or_default();

    // Goto still crosses files on the warm start, which is the answer the cache exists to preserve.
    server.open("src/Service.php").await;
    let target = server
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": server.uri("src/Service.php") },
                "position": { "line": 7, "character": 20 },
            }),
        )
        .await;

    expect!["phrost indexed 2 of 2 discovered files (0 reused from cache, 0 of them without reading, index written), framework none detected"]
        .assert_eq(&cold);
    expect![
        "phrost indexed 2 of 2 discovered files (2 reused from cache, 2 of them without reading), framework none detected"
    ]
    .assert_eq(&warm);
    expect!["src/Repo.php"].assert_eq(&cold_target);
    expect!["src/Repo.php"]
        .assert_eq(&definition_target(&target, server.project.root()).unwrap_or_default());
}
