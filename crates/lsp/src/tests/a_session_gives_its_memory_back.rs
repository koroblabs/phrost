//! What a session holds after doing the same thing many times.
//!
//! A leak in a language server is rarely memory nobody can reach -- Rust frees that -- but memory
//! something still reaches and nothing will ever read again: a list appended to on every save, a
//! task that outlives its server. No allocator prevents one. `lspkit_heap::Heap` counts what is
//! live, and these tests are the other half: drive one thing a session does over and over, and
//! require that the count stops moving.
//!
//! Each test warms up first, because what a first time builds -- an interned name, a table's room --
//! is a cost and not a leak. Then it runs the same steps twice as many times again, and the second
//! run may not end holding more than the first by more than [`SLACK`]. A leak grows with the number
//! of times; a one-time cost does not.
//!
//! What they found on the day they were written: 83 bytes held for every file changed on disk, for
//! the life of the server (`Workspace::watched_since_discovery`), and 6.5 KB left by every server
//! built and dropped (lspkit's socket kept open the channel its own stream read).
//!
//! Two things make a reading worth believing, and both were learned by getting it wrong:
//!
//! - **The count is the whole process's**, so a test runs alone -- see [`alone`].
//! - **Nothing the test keeps may allocate inside the loop.** The first draft noted each step's
//!   reading in a `String` and measured itself: 8 bytes a step, with a jump each time its own
//!   vector doubled, exactly the shape of a leak. The readings go into room taken before the loop,
//!   and every step ends with `Flow::forget`, because the stand-in client records every message the
//!   server sends.

use std::process::Command;

use serde_json::json;

use super::Fixture;

/// A project small enough to cycle quickly and large enough to exercise what a session does: a
/// call across two files, a problem the project check reports, and an autoload map.
const PROJECT: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /vendor/composer/autoload_psr4.php
<?php
$vendorDir = dirname(__DIR__);
$baseDir = dirname($vendorDir);
return array(
    'App\\' => array($baseDir . '/src'),
);

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): ?Repo { return null; }
}

//- /src/Service.php
<?php
namespace App;

class Service
{
    public function run(Repo $repo): void
    {
        $repo->f@at@ind(1);
        $repo->fnid(1);
    }
}
"#;

/// How much more the second run may end holding than the first.
///
/// Measured with both leaks fixed: the second run ends holding no more than the first in all four
/// tests, under nextest and under `cargo test` alike -- they pass at a slack of 0. A kilobyte is
/// room for a table somewhere to double once late, and no more: a leak of 21 bytes a step over
/// fifty steps is past it, and the one this file found first was 83.
const SLACK: usize = 1024;

/// How many readings at the end of each run its floor is the least of, so that a moment's
/// allocation -- a blocking thread starting, a task between its two halves -- is not read as what
/// the run kept. Measured: one 24 KB spike that was gone a step later, in sixty server restarts.
const FLOOR: usize = 5;

/// The variable a test sets on the copy of itself it runs alone.
const ALONE: &str = "PHROST_TEST_ALONE";

/// Whether this process is running nothing but the calling test -- after making sure one does.
///
/// Live bytes are counted for the whole process, so a test beside others would count what they
/// allocate. nextest gives every test a process of its own. `cargo test` runs them as threads of
/// one, and there the test runs itself again in a child with only itself selected, and this answers
/// `false` so the caller returns: the child's verdict is the test's.
fn alone(test: &str) -> bool {
    let nextest =
        std::env::var_os("NEXTEST_EXECUTION_MODE").is_some_and(|mode| mode == "process-per-test");
    if nextest || std::env::var_os(ALONE).is_some() {
        return true;
    }
    // `module_path!` names the crate; the test harness does not.
    let test = test.split_once("::").map_or(test, |(_, path)| path);
    let status = Command::new(std::env::current_exe().expect("a test binary has a path"))
        .args([test, "--exact", "--test-threads=1"])
        .env(ALONE, "1")
        .status()
        .expect("the test binary can run again");
    assert!(status.success(), "{test} failed when run alone");
    false
}

/// The live bytes after every step, kept in room taken before the first one.
struct Readings {
    warm: usize,
    run: usize,
    live: Vec<usize>,
}

impl Readings {
    fn new(warm: usize, run: usize) -> Readings {
        Readings {
            warm,
            run,
            live: Vec::with_capacity(warm + 2 * run),
        }
    }

    /// How many steps to take.
    fn steps(&self) -> usize {
        self.warm + 2 * self.run
    }

    /// Notes what is live now. Allocates nothing: the room was taken up front.
    fn note(&mut self) {
        self.live.push(lspkit_heap::stats().live());
    }

    /// The least of the last [`FLOOR`] readings of the run that ends at step `end`.
    fn floor(&self, end: usize) -> usize {
        self.live[end - FLOOR..end]
            .iter()
            .copied()
            .min()
            .expect("a run is longer than its floor")
    }

    fn assert_steady(&self, doing: &str) {
        assert_eq!(self.live.len(), self.steps(), "every step is noted");
        let first = self.floor(self.warm + self.run);
        let second = self.floor(self.steps());
        assert!(
            second <= first + SLACK,
            "{doing} {} more times held {} bytes more than the {} times before ({first} -> \
             {second}): something is kept that grows with how often it is done",
            self.run,
            second - first,
            self.run,
        );
    }
}

/// One of two texts, by step, so the steps are not all the same edit -- and so the first run and
/// the second end on the same one.
fn alternate(text: &str, step: usize, from: &str, to: &str) -> String {
    if step.is_multiple_of(2) {
        text.replace(from, to)
    } else {
        text.to_owned()
    }
}

/// Everything a person does to one file in an editor: open it, type, ask, close it.
#[tokio::test(start_paused = true)]
async fn a_buffer_opened_edited_and_closed_over_and_over_holds_no_more() {
    if !alone(concat!(
        module_path!(),
        "::a_buffer_opened_edited_and_closed_over_and_over_holds_no_more"
    )) {
        return;
    }
    let mut server = Fixture::new(PROJECT);
    server.initialize(false, true).await;
    server.initialized().await;
    let text = server.text("src/Service.php");
    let at = json!({
        "textDocument": { "uri": server.uri("src/Service.php") },
        "position": server.position("src/Service.php", "at"),
    });

    let mut readings = Readings::new(20, 50);
    for step in 0..readings.steps() {
        server.open("src/Service.php").await;
        for version in 2..4 {
            let typed = alternate(&text, step + version, "fnid", "find");
            server
                .change("src/Service.php", version as i64, &typed)
                .await;
            server.wait_out_the_quiet().await;
            server.hover("src/Service.php", "at").await;
            server.definition("src/Service.php", "at").await;
            server.request("textDocument/completion", at.clone()).await;
            server.semantic_tokens("src/Service.php").await;
            server.folding("src/Service.php").await;
        }
        server.close("src/Service.php").await;
        server.wait_out_the_quiet().await;
        server.flow.forget();
        readings.note();
    }
    readings.assert_steady("opening, editing and closing a buffer");
}

/// A save, as the watcher reports it: the file indexed again and the project checked again.
#[tokio::test(start_paused = true)]
async fn a_file_changed_on_disk_over_and_over_holds_no_more() {
    if !alone(concat!(
        module_path!(),
        "::a_file_changed_on_disk_over_and_over_holds_no_more"
    )) {
        return;
    }
    let mut server = Fixture::new(PROJECT);
    server.initialize(false, true).await;
    server.initialized().await;
    let text = server.text("src/Repo.php");

    let mut readings = Readings::new(20, 50);
    for step in 0..readings.steps() {
        let project = server.watch_project_passes();
        let saved = alternate(&text, step, "find(int", "fnid(int");
        std::fs::write(server.project.path("src/Repo.php"), saved)
            .expect("the fixture is writable");
        server.notify_changed("src/Repo.php").await;
        server.project_pass_ends(project).await;
        server.flow.forget();
        readings.note();
    }
    readings.assert_steady("changing a file on disk");
}

/// A `composer.json` change, which throws the index away and builds it again.
#[tokio::test(start_paused = true)]
async fn the_project_indexed_over_and_over_holds_no_more() {
    if !alone(concat!(
        module_path!(),
        "::the_project_indexed_over_and_over_holds_no_more"
    )) {
        return;
    }
    let mut server = Fixture::new(PROJECT);
    server.initialize(false, true).await;
    server.initialized().await;
    let manifest = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}
"#;

    let mut readings = Readings::new(10, 20);
    for step in 0..readings.steps() {
        let passes = server.watch_passes();
        let written = alternate(manifest, step, r#""src"}"#, r#""src", "Other\\": "other"}"#);
        server.write_and_notify(&written).await;
        server.pass_ends(passes).await;
        server.flow.forget();
        readings.note();
    }
    readings.assert_steady("indexing the project again");
}

/// A server built, started, asked something and dropped -- which a process does once, and which a
/// test suite does hundreds of times. What one leaves behind is what a server holds that its own
/// drop does not reach: a task still running, a channel still open.
#[tokio::test(start_paused = true)]
async fn a_server_built_and_dropped_over_and_over_leaves_nothing() {
    if !alone(concat!(
        module_path!(),
        "::a_server_built_and_dropped_over_and_over_leaves_nothing"
    )) {
        return;
    }
    let mut server = Fixture::new(PROJECT);

    let mut readings = Readings::new(10, 20);
    for _ in 0..readings.steps() {
        server.restart(crate::Workspace::default());
        server.initialize(false, true).await;
        server.initialized().await;
        server.open("src/Service.php").await;
        server.hover("src/Service.php", "at").await;
        readings.note();
    }
    readings.assert_steady("building and dropping a server");
}
