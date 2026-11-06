//! Changes made outside the editor: what the client is asked to watch, and what a change it
//! reports does.
//!
//! The index is built once, and this is what keeps it describing the checkout on disk after a `git
//! switch`, a `composer update` or a file a build script wrote. Its own file because none of it is
//! about building the index -- it decides *when* to build it again, and a file that is read for one
//! of those questions should not have to be read for the other.

use std::{sync::atomic::Ordering, time::Duration};

use lspkit_runtime::client;
use lspkit_runtime::protocol::ls_types::*;

use crate::{
    bootstrap::Pass,
    server::Backend,
    workspace::{WatchedOutcome, composer::COMPOSER_RECORDS},
};

/// Registration id for the file watchers, so they can be replaced rather than duplicated.
const WATCHER_REGISTRATION: &str = "phrost/watched-files";

/// How often a task waiting out a composer run looks at whether the run has been recorded.
///
/// Two `stat`s a look, and only while a lock is newer than the record.
const COMPOSER_POLL: Duration = Duration::from_millis(500);

/// How long a lock is waited on before the project is read anyway.
///
/// A lock can arrive with no install behind it -- a `git pull` that brought somebody else's -- and
/// the packages on disk are then the ones the index already describes, so the wait costs nothing.
/// Bounded all the same, because an editor that reports the record is the only other thing that
/// ends it, and one that does not would otherwise leave a task looking for good.
const COMPOSER_PATIENCE: Duration = Duration::from_secs(600);

impl Backend {
    pub(crate) async fn register_file_watchers(&self) {
        if !self.supports_file_watching.load(Ordering::Relaxed) {
            self.client
                .log_message(
                    MessageType::WARNING,
                    "this client cannot watch files, so changes made outside the editor \
                     (a branch switch, a composer install) will not be picked up",
                )
                .await;
            return;
        }

        let (globs, records) = {
            let workspace = self.workspace.read().await;
            (workspace.watch_globs(), workspace.composer_records())
        };
        let mut watchers = globs
            .into_iter()
            .map(GlobPattern::String)
            .collect::<Vec<_>>();
        // Rooted where the client can root a watch, because that is what makes it watch an ignored
        // `vendor/` at all -- see `supports_rooted_watch`. The glob is the best a client that cannot
        // has to offer, and is what every client was sent before.
        let rooted = self
            .supports_rooted_watch
            .load(Ordering::Relaxed)
            .then(|| records.as_deref().and_then(Uri::from_file_path))
            .flatten();
        watchers.extend(COMPOSER_RECORDS.iter().map(|record| match &rooted {
            Some(directory) => GlobPattern::Relative(RelativePattern {
                base_uri: OneOf::Right(directory.clone()),
                pattern: (*record).to_owned(),
            }),
            None => GlobPattern::String(format!("**/composer/{record}")),
        }));
        let watchers = watchers
            .into_iter()
            .map(|glob_pattern| FileSystemWatcher {
                glob_pattern,
                kind: None,
            })
            .collect();

        client::watch_files(
            &self.client,
            WATCHER_REGISTRATION,
            "watch the project for changes made outside the editor",
            watchers,
        )
        .await;
    }

    pub(crate) async fn apply_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let mut reindexed = 0_usize;
        let mut removed = 0_usize;
        let mut configuration_changed = false;
        let mut composer_installing = false;

        {
            let mut workspace = self.workspace_write().await;
            let mut composer_changed = false;
            for change in &params.changes {
                let Some(path) = change.uri.to_file_path() else {
                    continue;
                };
                match workspace.apply_watched_change(&path, change.typ == FileChangeType::DELETED) {
                    WatchedOutcome::Reindexed => reindexed += 1,
                    WatchedOutcome::Removed => removed += 1,
                    WatchedOutcome::ConfigurationChanged => configuration_changed = true,
                    WatchedOutcome::ComposerChanged => composer_changed = true,
                    WatchedOutcome::Ignored => {}
                }
            }
            // A lock newer than the install record is composer's first write of a run, and the
            // packages it announces are being replaced from that moment on. Reading the project now
            // reads them half replaced -- so the run is waited out, and until it ends the index
            // that was right before it is the better answer.
            if composer_changed {
                composer_installing = workspace.composer_stamp().installing();
                configuration_changed |= !composer_installing;
            }
        }

        if configuration_changed {
            self.reconfigure().await;
            return;
        }
        if composer_installing {
            self.wait_for_composer().await;
        }

        if reindexed > 0 || removed > 0 {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("reindexed {reindexed} and dropped {removed} changed files"),
                )
                .await;
            // What changed on disk may have changed what is wrong elsewhere -- a signature edited
            // in one file is a call broken in another -- so the whole project is checked again,
            // once the burst of changes has settled.
            self.check_project_in_the_background(crate::project::AFTER_CHANGES);
            // And the open buffers, which that pass deliberately skips: it checks the files nobody
            // has open. Without this the one file the person is looking at is the only file in the
            // project still showing what was wrong before the change -- a method added by a
            // colleague's commit, or by a `composer update`, stays underlined in the editor and
            // nowhere else, and nothing but a restart or a keystroke in that buffer corrects it.
            self.check_open_documents_in_the_background();
            // And the counts beside them: a file that appeared is references a class did not have,
            // and the lens saying so is drawn over a file nobody touched. One request for the
            // whole burst, which is what the client's refresh is for.
            self.refresh_code_lenses().await;
        }
    }

    /// Reads the project again from nothing, because what decides where its code lives changed.
    async fn reconfigure(&self) {
        let (root, stamp) = {
            let workspace = self.workspace.read().await;
            (
                workspace.layout().map(|layout| layout.root().to_path_buf()),
                workspace.composer_stamp(),
            )
        };
        let Some(root) = root else {
            return;
        };
        *self
            .reconfigured_for
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(stamp);
        // Here rather than only in the pass, which may queue behind one still running: where the
        // code lives has just changed, so from this instant the index describes a layout that is
        // gone and refusing is the honest answer.
        self.indexed.store(false, Ordering::Relaxed);
        self.client
            .log_message(
                MessageType::INFO,
                "project configuration changed; reindexing the project",
            )
            .await;
        // Off the handler for the same reason startup is: this is a full reindex of the project,
        // and a notification handler that runs one stops every other request for as long as it
        // takes.
        self.index_in_the_background(Pass::Reconfigured(root));
    }

    /// Reads the project again once composer has recorded the install its lock announced.
    ///
    /// Looked for rather than only waited for. An editor that roots its watches reports the record
    /// and starts the read itself; one that does not -- or one that dropped the event, which macOS
    /// does under a burst of thousands of writes -- would leave the index describing packages that
    /// are gone, and nothing short of a restart would correct it. One task however many locks
    /// arrive, and none once a read has been started against the finished run.
    async fn wait_for_composer(&self) {
        if self.waiting_for_composer.swap(true, Ordering::Relaxed) {
            return;
        }
        self.client
            .log_message(
                MessageType::INFO,
                "composer is installing; the project is read again once it has finished",
            )
            .await;
        let backend = self.clone();
        tokio::spawn(async move {
            let waited = tokio::time::Instant::now();
            loop {
                tokio::time::sleep(COMPOSER_POLL).await;
                let stamp = backend.workspace.read().await.composer_stamp();
                if stamp.installing() && waited.elapsed() < COMPOSER_PATIENCE {
                    continue;
                }
                // Cleared before the read is started, so a lock that arrives in between starts a
                // wait of its own rather than finding this one still claiming the job.
                backend.waiting_for_composer.store(false, Ordering::Relaxed);
                let already = *backend
                    .reconfigured_for
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    == Some(stamp);
                if !already {
                    backend.reconfigure().await;
                }
                return;
            }
        });
    }
}
