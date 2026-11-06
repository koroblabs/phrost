//! Building the index the server answers from.
//!
//! None of this is protocol. It is what the server does to the project before the first request and
//! whenever it has to be read again, so what stays beside the handlers is the line that calls in
//! here. What decides that it has to be read again is `watching`.

use std::{future::Future, path::PathBuf, sync::atomic::Ordering, time::Duration};

use lspkit_runtime::client;
use lspkit_runtime::panics::{self, Panic};
use lspkit_runtime::protocol::ls_types::*;

use crate::server::Backend;

/// How many files one bootstrap batch indexes before releasing the lock.
///
/// The index's own batch size, because a batch is the unit of parallelism and the lock is released
/// between batches: one number decides both how long a request arriving mid-bootstrap waits and
/// how wide a parallel slice is. See [`phrost_index::INGEST_BATCH`] for why it is what it is.
use phrost_index::INGEST_BATCH as BOOTSTRAP_BATCH;

/// Progress token for the initial index.
///
/// A fixed string rather than a counter: there is only ever one bootstrap, and a stable token lets a
/// client recognise the same operation across a restart.
const BOOTSTRAP_TOKEN: &str = "phrost/bootstrap";

const HIERARCHY_REGISTRATION: &str = "phrost/type-hierarchy";

/// Why the project is being indexed, which is the only thing the two passes differ by.
#[derive(Clone, Debug)]
pub(crate) enum Pass {
    /// The first one. The client is asked for what only a registration can offer, then the project
    /// is indexed.
    Startup,
    /// Composer's configuration changed, so where the code lives has to be found again from nothing.
    Reconfigured(PathBuf),
}

impl Backend {
    /// Runs an indexing pass on its own task, and hands the message loop straight back.
    ///
    /// Awaiting one here is what made a fresh server look hung. `initialized` is a notification, and
    /// notifications are finished in order before the next message is read -- under
    /// `tower-lsp-server`, which polled every request future in the same task, and under lspkit's
    /// own loop, which keeps the ordering on purpose -- so nothing else advanced while bootstrap ran. Measured on a
    /// 30 000-file project: a hover issued straight after `didOpen` took 5 465 ms and was *answered*,
    /// which is the proof rather than the symptom. Had its future been polled at all,
    /// `Session::require_ready` would have refused it in microseconds. A gate cannot refuse a
    /// request nobody is looking at.
    ///
    /// The task inherits a worker thread, and the runtime is built with a 64 MiB
    /// `thread_stack_size` -- so a pass has the same stack a request handler does.
    pub(crate) fn index_in_the_background(&self, pass: Pass) {
        let backend = self.clone();
        tokio::spawn(async move { backend.indexing_pass(backend.index(pass)).await });
    }

    /// One pass, serialized against every other, contained, and announced when it ends.
    ///
    /// The work is a parameter rather than a branch so the containment can be tested with work that
    /// is guaranteed to fall over; nothing else needs it.
    ///
    /// A panic here reaches neither the loop's containment, which covers what runs on a request's
    /// own task, nor the process, which tokio keeps alive by holding the payload in a `JoinHandle` nobody
    /// reads. Uncaught it would leave a server refusing every request with "still indexing" for the
    /// rest of the session -- a permanent lie that reads exactly like a slow project, which is worse
    /// than the crash it replaced.
    pub(crate) async fn indexing_pass(&self, work: impl Future<Output = ()> + Send) {
        // A second pass queues behind the first rather than resetting the index under it. Only
        // reachable now that the loop keeps turning: before this, a watched-file notification could
        // not be read until startup had finished.
        let _one_at_a_time = self.indexing.lock().await;

        // Cleared first, because what the refusal says has to be about *this* pass: a project whose
        // configuration changed after a pass fell over is being indexed again, not still broken.
        self.failed.store(false, Ordering::Relaxed);

        if let Err(panicked) = Panic::watching(work).await {
            self.failed.store(true, Ordering::Relaxed);
            panicked.report(&self.client, "indexing the project").await;
            // The project check that a finished bootstrap would have started is not coming, and
            // whoever waits for the end of one -- the tests do -- must not wait forever.
            self.finished_a_project_pass();
        }

        self.finished_a_pass();
    }

    /// What a pass actually does, by the reason for it.
    async fn index(&self, pass: Pass) {
        match pass {
            Pass::Startup => {
                self.register_file_watchers().await;
                self.register_type_hierarchy().await;
            }
            Pass::Reconfigured(root) => {
                let mut workspace = self.workspace_write().await;
                self.indexed.store(false, Ordering::Relaxed);
                workspace.reset_index();
                workspace.set_root(root);
            }
        }

        self.bootstrap().await;
    }

    pub(crate) async fn bootstrap(&self) {
        {
            let mut workspace = self.workspace_write().await;
            // Before discovery, because what was written last time is the index this run starts
            // from -- its ids, its interner and its type table are the ones its stored member rows
            // were written against, so it is loaded whole and then confirmed file by file.
            workspace.load_cache();
            // And immediately after, because loading adopts that stored skeleton wholesale: a
            // buffer the editor opened in the moment before is no longer in the index, while the
            // editor still shows it.
            workspace.reingest_open_documents();
        }

        // After loading this project's entry and before anything else: the directory is shared by
        // every project ever opened, and nothing else in a server's life is a good moment to tidy
        // it -- a server that is killed, which is how an editor session normally ends, never gets
        // one at all.
        let pruned = self.workspace.read().await.prune_cache();
        if pruned.removed > 0 {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "phrost cache: removed {} stale {}, freed {} MB, kept {}",
                        pruned.removed,
                        match pruned.removed {
                            1 => "index",
                            _ => "indexes",
                        },
                        pruned.freed / (1024 * 1024),
                        pruned.kept,
                    ),
                )
                .await;
        }

        let found = self.workspace.read().await.discover();
        let (discovered, configuration, templates) =
            (found.php, found.configuration, found.templates);
        let total = discovered.len();
        let mut reused = phrost_index::Reused::default();

        // Percentage of files, not of time. It is honest about what it measures, and a bar that
        // advances unevenly is still far better than no bar. A client that ignores the request for
        // a bar is not asked again for the project check after this.
        let mut progress = self
            .progress
            .start(&self.client, BOOTSTRAP_TOKEN, "Indexing", total, "files")
            .await;

        let mut done = 0_usize;
        for batch in discovered.chunks(BOOTSTRAP_BATCH) {
            {
                let mut workspace = self.workspace_write().await;
                reused += workspace.ingest_batch(batch, true);
            }
            done += batch.len();
            progress.reached(done).await;
        }

        let indexed = {
            let mut workspace = self.workspace_write().await;
            // A file the cache described and discovery did not is gone, and has to leave the catalog
            // with it.
            workspace.forget_undiscovered(&discovered);
            // Last, because the two steps above and the batch loop between them all reach an open
            // buffer the wrong way: the loop reads it from disk and so misses unsaved edits, and
            // both of the others can drop it outright. The editor is the authority on a file it has
            // open, and this is where the index is made to agree with it again.
            workspace.reingest_open_documents();
            // After the index, because a configuration file names classes and there is nothing to
            // check them against until the classes are in -- and before `mark_ready`, so the first
            // request never sees a project whose configuration references are half read.
            workspace.install_config_sites(&configuration);
            workspace.install_template_words(&templates);
            workspace.mark_ready();
            // The lock-free copy, set inside the same critical section that sets the real one so
            // the two cannot disagree about a project that is half indexed.
            self.indexed.store(true, Ordering::Relaxed);
            workspace.indexed_file_count()
        };

        // Written after, and only when something was actually indexed: storing what was just loaded
        // unchanged is a few hundred milliseconds of writing for no difference at all.
        let stored = reused.total() < indexed && self.workspace.read().await.store_cache();
        // Everything bootstrap allocated and freed -- a parse arena per file above all -- is memory
        // the allocator would otherwise keep for the life of the process. Once, here, because this
        // is the only moment that much is given back at once; see `lspkit-heap`.
        lspkit_heap::release_freed();

        progress.finish(format!("{indexed} files indexed")).await;

        // With the frameworks, because a project whose framework was not recognised has every
        // service id, hook and generated route resolving to nothing -- which reads as the server
        // being broken rather than as one probe answering no, and the log is the only place a
        // person can see which it was. `(configured)` marks one the project switched on itself.
        let (frameworks, excluded, included) = {
            let workspace = self.workspace.read().await;
            (
                workspace.framework_summary(),
                workspace.excluded_count(),
                workspace.included_count(),
            )
        };
        // Only when a project said something, so the ordinary line stays one line.
        let paths = match (excluded, included) {
            (0, 0) => String::new(),
            (skipped, 0) => format!(", {skipped} path{} skipped", plural(skipped)),
            (0, kept) => format!(", {kept} path{} included", plural(kept)),
            (skipped, kept) => format!(
                ", {skipped} path{} skipped and {kept} included",
                plural(skipped)
            ),
        };
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "phrost indexed {indexed} of {total} discovered files ({} reused from cache, \
                     {} of them without reading{}), framework {frameworks}{paths}",
                    reused.total(),
                    reused.unread,
                    if stored { ", index written" } else { "" }
                ),
            )
            .await;

        // Said out loud, because the symptom of a missing standard library is every reference to
        // `strlen` reading as undeclared -- which looks like a resolution bug, not a missing asset.
        if let Some(reason) = self.workspace.read().await.stub_error() {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!(
                        "{reason} -- the PHP standard library will not resolve, so names like \
                         `strlen` and `\\Exception` will report as undeclared"
                    ),
                )
                .await;
        }

        // The same reason, and a worse symptom: a rule a project meant to switch off stays on, which
        // reads as the setting not working rather than as the source not being read. Each source that
        // failed says so on its own line, because fixing one and finding the other still broken is
        // the same surprise a second time.
        let config_errors = self.workspace.read().await.config_errors().to_vec();
        for reason in config_errors {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("{reason} -- it was ignored, so its settings are not in force"),
                )
                .await;
        }

        self.check_open_documents().await;
        self.refresh_code_lenses().await;
        // And then everything nobody has open, in its own task: this one is done, the index is
        // ready, and a request arriving now is answered while the project is checked behind it.
        self.check_project_in_the_background(Duration::ZERO);
    }

    /// Asks the client to re-request code lenses, now that there is something to count.
    ///
    /// A lens is answered with silence until the index is ready -- see `answers::lenses` -- and a
    /// client asks for the lenses of a file when it opens it and then not again. An editor
    /// restores its tabs the instant `initialized` goes out, so on every real start that ask is
    /// the early one, and without this nothing ever tells it to ask again: the lenses of every
    /// open file are simply absent until the file is edited, which is what makes it read as a
    /// cache rather than as a race.
    ///
    /// Only where the client said it would listen, which the specification requires, and bounded
    /// like every other server-to-client request here -- see [`client::ask`] for the start-up
    /// this once hung on.
    pub(crate) async fn refresh_code_lenses(&self) {
        if !self.supports_lens_refresh.load(Ordering::Relaxed) {
            return;
        }
        client::ask(
            &self.client,
            "refresh the reference counts",
            self.client.code_lens_refresh(),
        )
        .await;
    }

    /// The same, on a task of its own.
    ///
    /// For the callers that are notification handlers: checking a buffer is lowering and analysis,
    /// and the message loop polls every request future in one task -- so a handler that does it
    /// inline stops every other request for as long as the open files take.
    pub(crate) fn check_open_documents_in_the_background(&self) {
        let backend = self.clone();
        panics::spawn(
            self.client.clone(),
            || "checking the open files".to_owned(),
            async move { backend.check_open_documents().await },
        );
    }

    pub(crate) async fn check_open_documents(&self) {
        let open = self.workspace.read().await.open_documents();
        for (path, version) in open {
            if let Some(uri) = Uri::from_file_path(&path) {
                self.publish_diagnostics(&path, &uri, version.as_i32())
                    .await;
            }
        }
    }

    /// Offers the type-hierarchy requests, which have no static capability to offer them with.
    ///
    /// The specification defines none: a client issues `textDocument/prepareTypeHierarchy` only if the
    /// server has registered for it. So a server that fills in a capability field instead -- there is
    /// no field to fill -- looks to every client exactly like one that does not have the feature.
    pub(crate) async fn register_type_hierarchy(&self) {
        if !self.supports_type_hierarchy.load(Ordering::Relaxed) {
            return;
        }

        let registration = Registration {
            id: HIERARCHY_REGISTRATION.to_owned(),
            method: "textDocument/prepareTypeHierarchy".to_owned(),
            register_options: None,
        };

        client::ask(
            &self.client,
            "offer the class hierarchy",
            self.client.register_capability(vec![registration]),
        )
        .await;
    }
}

/// `s` unless there is one of them, for a count read in a sentence.
fn plural(count: usize) -> &'static str {
    match count {
        1 => "",
        _ => "s",
    }
}
