//! Checking every file the project owns, in the background, and saying how far along it is.
//!
//! The open buffers are checked as they are typed in; this is the rest. Once the index is ready,
//! and again whenever a file changes on disk, every file the project owns that nobody has open is
//! read, checked exactly as an open buffer would be, and published -- so the problems panel shows
//! the project and not the tabs, and a method renamed in one file lights up its callers in the
//! others without anybody opening them.
//!
//! What it deliberately does not do: publish "none" for a file that was never reported. A client
//! keeps only what it was sent, and thirty thousand notifications to say nothing is not a way to
//! start a session. The session remembers which files it reported problems in and clears exactly
//! those when a later pass finds them clean.

use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use lspkit_runtime::panics::Panic;
use lspkit_runtime::protocol::ls_types::*;

use crate::server::Backend;

const PROJECT_TOKEN: &str = "phrost/project-diagnostics";

/// How long after a change on disk the project is checked again.
///
/// Longer than a keystroke's quiet, because a change on disk is rarely one: a save is one event and
/// a branch switch is hundreds, and one pass should cover the lot.
pub(crate) const AFTER_CHANGES: Duration = Duration::from_secs(1);

/// How many threads check files at once.
///
/// Measured on the reference checkout's 1 711 files, warm:
///
/// | workers | pass | resident after |
/// | ---: | ---: | ---: |
/// | 1 | 1.15 s | 168-172 MB |
/// | 2 | 0.60 s | 170-171 MB |
/// | 4 | 0.32 s | 173-175 MB |
/// | 8 | 0.21 s | 175-178 MB |
///
/// Memory barely moves, because a project's own files are small and their arenas are gone before
/// the next one is read; what tapers is the speed. Four, as the indexer settled on and for the same
/// reason: the rest of the machine is somebody's editor, and the pass runs while they type.
const CHECK_WORKERS: usize = 4;

impl Backend {
    /// Checks every closed file the project owns, once the delay has passed with no newer ask.
    ///
    /// Each ask supersedes the last: a pass reads the counter between files and stops when it is no
    /// longer the current one, so a save landing mid-pass costs one file and not two passes.
    pub(crate) fn check_project_in_the_background(&self, after: Duration) {
        let generation = self.project_pass.fetch_add(1, Ordering::Relaxed) + 1;
        let backend = self.clone();
        tokio::spawn(async move {
            if !after.is_zero() {
                tokio::time::sleep(after).await;
            }
            if backend.project_pass.load(Ordering::Relaxed) == generation
                && let Err(panicked) = Panic::watching(backend.check_project(generation)).await
            {
                panicked
                    .report(&backend.client, "checking the project")
                    .await;
            }
            backend.finished_a_project_pass();
        });
    }

    async fn check_project(&self, generation: u64) {
        let files = {
            let workspace = self.workspace.read().await;
            if !workspace.diagnoses_project() || !self.indexed.load(Ordering::Relaxed) {
                return;
            }
            workspace.own_closed_files()
        };
        let total = files.len();
        // The bar for the check. Not asked for again from a client that ignored the request for the
        // indexing one: the wait for its answer was the whole cost, and it will not answer this one
        // either.
        let mut progress = self
            .progress
            .start(&self.client, PROJECT_TOKEN, "Checking", total, "files")
            .await;

        // The files are checked on threads of their own and published from here as each finishes.
        // Checking is lowering and analysis, which is processor time and not waiting; on the
        // runtime's workers it would take one of them for the length of the pass, and on one thread
        // it took a second for 1 711 files where four take a quarter of that. Each worker takes the
        // next file nobody has taken, so a long file balances against short ones rather than
        // leaving its thread the last one running.
        let (send, mut receive) = tokio::sync::mpsc::channel(CHECK_WORKERS * 2);
        let checker = self.clone();
        let workers = tokio::task::spawn_blocking(move || {
            let next = AtomicUsize::new(0);
            std::thread::scope(|scope| {
                for _ in 0..CHECK_WORKERS.min(files.len()).max(1) {
                    let send = send.clone();
                    let (files, next, checker) = (&files, &next, &checker);
                    // With a stack, and this is the site where it mattered most: the pass checks
                    // every file the project owns, so it lowers bodies nobody opened. Measured on
                    // the default: a 4 000-deep method chain in a file the person had never
                    // touched aborted the whole server -- exit -6, no error, nothing to restart
                    // from, because a stack overflow is not an unwind and no panic containment
                    // can see it.
                    std::thread::Builder::new()
                        .name("phrost-check".to_owned())
                        .stack_size(phrost_model::DEEP_RECURSION_STACK)
                        .spawn_scoped(scope, move || {
                            loop {
                                let index = next.fetch_add(1, Ordering::Relaxed);
                                let Some((path, _)) = files.get(index) else {
                                    break;
                                };
                                // Superseded by a newer ask, or the index is being rebuilt under this
                                // pass -- in which case the bootstrap rebuilding it starts a fresh one.
                                if checker.project_pass.load(Ordering::Relaxed) != generation
                                    || !checker.indexed.load(Ordering::Relaxed)
                                {
                                    break;
                                }
                                let found =
                                    Self::findings_from_disk_blocking(&checker.workspace, path);
                                // A closed channel means the publisher is gone, which only happens if
                                // it panicked. Stop rather than spin.
                                if send.blocking_send((path.clone(), found)).is_err() {
                                    break;
                                }
                            }
                        })
                        .expect("the operating system can start a check thread");
                }
            });
        });

        let mut done = 0_usize;
        let mut checked = 0_usize;
        let mut problems = 0_usize;
        let mut reported = 0_usize;
        while let Some((path, found)) = receive.recv().await {
            done += 1;
            if let Some(found) = found
                && let Some(uri) = Uri::from_file_path(&path)
            {
                checked += 1;
                if self.note_project_findings(&path, !found.is_empty()) {
                    problems += found.len();
                    reported += 1;
                    self.client.publish_diagnostics(uri, found, None).await;
                }
            }
            progress.reached(done).await;
        }
        // The workers are done once the channel is: every sender is dropped with the scope. A
        // panic on one of them surfaces here rather than being swallowed with the task.
        if let Err(error) = workers.await
            && error.is_panic()
        {
            self.client
                .log_message(
                    MessageType::ERROR,
                    "phrost failed while checking the project -- the rest of the files were not \
                     checked; a change on disk starts the check again",
                )
                .await;
        }

        // Every closed file has now had its say, and the open ones say theirs on every publish --
        // so what nothing was seen to reach really is reached by nothing, and the unused check can
        // stop answering from what names are merely *written* somewhere. Only when the pass ran to
        // the end: a superseded one covered a prefix of the project, and a prefix is not an
        // answer. The buffers are republished once, because their findings just got sharper and
        // nothing else will tell them so.
        if done == total && self.project_pass.load(Ordering::Relaxed) == generation {
            let (sharpened, read) = {
                let workspace = self.workspace.read().await;
                (
                    workspace.mark_reached_complete(),
                    workspace.reads_what_is_reached(),
                )
            };
            if sharpened && read {
                // Gather, then judge. Every file this pass published was judged while the record
                // was still being filled -- by the coarse rule, because that is all there was --
                // so the answers it gave are a subset of the ones it can give now. One more sweep
                // says the rest. Only ever one: the record is complete from here on, so the next
                // `mark_reached_complete` changes nothing and sweeps nothing.
                //
                // Inside this pass rather than scheduled as another, so that one pass is one
                // thing: a caller waiting for the pass to end -- every flow test does -- waits
                // for the answer it will keep, not for a first draft of it.
                self.check_open_documents().await;
                Box::pin(self.check_project(generation)).await;
            }
        }

        progress
            .finish(format!("{problems} problems in {checked} files"))
            .await;
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "phrost checked {checked} of {total} files: {problems} problems in {reported} \
                     files"
                ),
            )
            .await;
    }
}
