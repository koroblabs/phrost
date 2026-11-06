//! What happens to a buffer the editor opens, changes, or closes.
//!
//! One place, because the two things a document event does are inseparable: the index and the open
//! set are updated from the same string, so the two tiers cannot come to describe different
//! revisions of one file -- and the file is then checked from that same revision.

use std::{
    path::Path,
    sync::{Arc, atomic::Ordering},
};

use lspkit_runtime::panics::Panic;
use lspkit_runtime::protocol::ls_types::*;
use lspkit_runtime::quiet;
use phrost_model::{FileId, LineIndex};
use phrost_vfs::{DocumentVersion, SourceText};
use tokio::sync::RwLock;

use crate::{Workspace, convert, server::Backend, workspace::DocumentUpdate};

/// When to publish what a change produced.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Publish {
    /// Straight away: an open, or a file that changed on disk.
    Now,
    /// Once the buffer has been still for [`quiet::QUIET`].
    WhenQuiet,
}

impl Backend {
    pub(crate) async fn store_document(
        &self,
        path: &Path,
        uri: &Uri,
        text: String,
        version: i32,
        publish: Publish,
    ) {
        let update = self.workspace_write().await.set_document(
            path,
            Arc::from(text),
            DocumentVersion::new(version),
        );

        match update {
            // The lock-free copy, not the skeleton's own state: an indexing pass now runs beside
            // this handler rather than before it, so asking the index would mean waiting for a
            // batch of it to answer a question an atomic answers.
            DocumentUpdate::Applied if self.indexed.load(Ordering::Relaxed) => match publish {
                Publish::Now => self.publish_diagnostics(path, uri, version).await,
                Publish::WhenQuiet => self.publish_when_quiet(path, uri, version),
            },
            // Not while the project is still being indexed. Every other cross-file answer refuses
            // until the catalog is complete, for the reason this one has to as well: half an index
            // reports half a project as broken, and a reader cannot tell that from a real finding.
            // What is open gets checked when bootstrap finishes.
            DocumentUpdate::Applied => {}
            // Silently: the client was configured to send this server a language it does not read,
            // which is the client's business and not a fault to report.
            DocumentUpdate::Ignored => {}
            // Held, and checkable: a key its framework's catalog does not hold. Published on the
            // same schedule PHP is, because a person editing a `services.yml` is typing into it the
            // same way and a squiggle that flickers is worse than one that arrives a moment late.
            DocumentUpdate::Held if self.indexed.load(Ordering::Relaxed) => match publish {
                Publish::Now => self.publish_diagnostics(path, uri, version).await,
                Publish::WhenQuiet => self.publish_when_quiet(path, uri, version),
            },
            DocumentUpdate::Held => {}
            DocumentUpdate::Stale { held } => {
                // Said out loud rather than swallowed: it means the client and this server disagree
                // about the order of edits, and every answer after it would be about the wrong text.
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!(
                            "ignored version {version} of {} because version {} is already held",
                            path.display(),
                            held.as_i32()
                        ),
                    )
                    .await;
            }
        }
    }

    /// Publishes once the buffer has stopped changing, and not at all if it has not -- see
    /// [`quiet::when_quiet`], which also reports a panic in the check rather than losing it with the
    /// task: lost, the buffer would never get findings again, which reads exactly like a file with
    /// nothing wrong with it.
    fn publish_when_quiet(&self, path: &Path, uri: &Uri, version: i32) {
        let client = self.client.clone();
        let workspace = Arc::clone(&self.workspace);
        // Shared by the three closures, so a keystroke pays for one copy of the path, not three.
        let path: Arc<Path> = Arc::from(path);
        let uri = uri.clone();
        let (named, asked, held) = (Arc::clone(&path), Arc::clone(&path), Arc::clone(&workspace));

        quiet::when_quiet(
            self.client.clone(),
            move || format!("checking {}", named.display()),
            version,
            move || async move {
                held.read()
                    .await
                    .version_at(&asked)
                    .map(DocumentVersion::as_i32)
            },
            move || async move {
                let found = Self::findings(&workspace, &path).await;
                client.publish_diagnostics(uri, found, Some(version)).await;

                // And every other open buffer, because an edit to one changes what the others see:
                // a method added in one file resolves in another the moment it is ingested, and
                // nothing told the client so. The other buffer kept the findings it had when it was
                // last touched, typing a character in it cured them, and a fresh request on it
                // already answered correctly -- so only the *published* set was stale, which is
                // what made this read as a diagnostics defect rather than a publishing one.
                //
                // On the same wait as the edit's own publish, so a burst of keystrokes costs one
                // pass rather than one per character; and after it, so the buffer somebody is
                // looking at is never behind the ones they are not.
                Self::publish_others(&client, &workspace, &path, version).await;
            },
        );
    }

    /// Publishes findings for every open document but one, at whatever version each is held at.
    ///
    /// Each is published at the version it is held at rather than at some guard of its own: they
    /// have not changed, so their versions are current by definition, and what moved underneath them
    /// is the index.
    ///
    /// The *edited* buffer's version is watched, though, and between files rather than only at the
    /// start: a person typing through this schedules a pass of their own, and finishing an obsolete
    /// one would spend the whole of it on an index that has already moved. Between files and not
    /// mid-file, because a file's findings are one answer and half of one is worth nothing.
    async fn publish_others(
        client: &lspkit_runtime::protocol::Client,
        workspace: &Arc<RwLock<Workspace>>,
        edited: &Path,
        version: i32,
    ) {
        let open = workspace.read().await.open_documents();
        for (path, held) in open {
            if path.as_path() == edited {
                continue;
            }
            let current = workspace.read().await.version_at(edited);
            if current.map(DocumentVersion::as_i32) != Some(version) {
                return;
            }
            let Some(uri) = Uri::from_file_path(&path) else {
                continue;
            };
            let found = match Panic::watching(Self::findings(workspace, &path)).await {
                Ok(found) => found,
                Err(panicked) => {
                    panicked
                        .report(client, &format!("checking {}", path.display()))
                        .await;
                    continue;
                }
            };
            client
                .publish_diagnostics(uri, found, Some(held.as_i32()))
                .await;
        }
    }

    pub(crate) async fn publish_diagnostics(&self, path: &Path, uri: &Uri, version: i32) {
        let found = Self::findings(&self.workspace, path).await;
        self.client
            .publish_diagnostics(uri.clone(), found, Some(version))
            .await;
    }

    /// What checking one open buffer produces, as the protocol's shape.
    ///
    /// An associated function over the lock rather than a method, because the waiting task holds a
    /// clone of the lock and not a `Backend`.
    async fn findings(
        workspace: &RwLock<crate::workspace::Workspace>,
        path: &Path,
    ) -> Vec<Diagnostic> {
        let workspace = workspace.read().await;
        if let Some(text) = workspace.config_text(path) {
            // A dependency's file opened to be read is not checked, for the reason given below for
            // one of its PHP files: core's templates and a contributed module's services are not
            // something the person reading them can act on.
            if !workspace.owns_path(path) {
                return Vec::new();
            }
            return config_findings(&workspace, path, text);
        }
        let Some(file) = workspace.file_id(path) else {
            return Vec::new();
        };
        // A dependency or a stub opened to be read is not checked, the same rule the project pass
        // and a closed file follow: nothing reported about symfony's docblocks or a stub's `__get`
        // is something the person reading them can act on, and `check` never counts them either.
        if !workspace.is_own_code(file) {
            return Vec::new();
        }
        let Some((text, lines)) = workspace.document_text(file) else {
            return Vec::new();
        };
        let view = workspace.view(file);
        // The other half of the record: an open buffer is not read from disk by the pass, so what
        // it reaches would be missing -- and a method called only from the tab beside the one you
        // are reading would look dead. Recorded from the buffer, so it is what is on screen.
        workspace.record_reached(file, view.reaches());
        let mut found = phrost_analysis::diagnostics_asked_for(&view, workspace.asked_for());
        // From what was written: the buffer held for a template is the PHP made of it.
        let written = workspace
            .written_template(file)
            .unwrap_or_else(|| Arc::clone(&text));
        found.extend(phrost_analysis::template_diagnostics(
            &view,
            &written,
            workspace.asked_for(),
        ));
        lsp_diagnostics(&workspace, file, &text, &lines, found)
    }

    /// The same for a file nobody has open, from what the disk holds.
    ///
    /// `None` for a file this server does not check -- not indexed, not the project's own, or not
    /// readable -- so a caller can tell "nothing wrong" from "nothing to say". Read once here for
    /// the positions and handed to the view as text, so the file is not read twice.
    pub(crate) async fn findings_from_disk(
        workspace: &RwLock<crate::workspace::Workspace>,
        path: &Path,
    ) -> Option<Vec<Diagnostic>> {
        let workspace = workspace.read().await;
        closed_findings(&workspace, path)
    }

    /// The same, from a thread that is not on the runtime -- a worker of the project-wide check.
    ///
    /// The lock is taken for one file and released, so an edit arriving while the project is
    /// checked waits for one file's worth of work and not for the pass.
    pub(crate) fn findings_from_disk_blocking(
        workspace: &RwLock<crate::workspace::Workspace>,
        path: &Path,
    ) -> Option<Vec<Diagnostic>> {
        let workspace = workspace.blocking_read();
        closed_findings(&workspace, path)
    }
}

fn closed_findings(
    workspace: &crate::workspace::Workspace,
    path: &Path,
) -> Option<Vec<Diagnostic>> {
    let file = workspace.file_id(path)?;
    if !workspace.is_own_code(file) {
        return None;
    }
    let source = SourceText::read(path).ok()?;
    let text = source.shared_text();
    let lines = LineIndex::new(&text);
    let found = workspace.analysed_from(file, Arc::clone(&text), |view| {
        // What this file reaches, recorded while its analysis is in hand. The pass that calls
        // this covers every closed file the project owns, which is what makes the record whole.
        workspace.record_reached(file, view.reaches());
        let mut found = phrost_analysis::diagnostics_asked_for(view, workspace.asked_for());
        found.extend(phrost_analysis::template_diagnostics(
            view,
            &text,
            workspace.asked_for(),
        ));
        found
    });
    Some(lsp_diagnostics(workspace, file, &text, &lines, found))
}

/// The index's findings and the syntax rules' as the protocol wants them, from one revision's text.
///
/// One function for a buffer and for a file on disk, so that opening a file never changes what is
/// reported about it.
fn lsp_diagnostics(
    workspace: &crate::workspace::Workspace,
    file: FileId,
    text: &Arc<str>,
    lines: &LineIndex,
    found: Vec<phrost_analysis::Diagnostic>,
) -> Vec<Diagnostic> {
    // Not over a template, whose text is markup: what is PHP in it was read through the view.
    let linted = match workspace.is_template(workspace.path_of(file)) {
        true => Vec::new(),
        false => phrost_syntax_features::lint::lint(
            text,
            &workspace.path_of(file).display().to_string(),
            workspace.frameworks(),
            workspace.editor_lint(),
        ),
    }
    .into_iter()
    .map(|found| Diagnostic {
        range: convert::range_of(text, lines, found.range),
        severity: Some(match found.severity {
            phrost_syntax_features::lint::LintSeverity::Error => DiagnosticSeverity::ERROR,
            phrost_syntax_features::lint::LintSeverity::Warning => DiagnosticSeverity::WARNING,
            phrost_syntax_features::lint::LintSeverity::Hint => DiagnosticSeverity::HINT,
        }),
        code: Some(NumberOrString::String(found.code)),
        source: Some("phrost lint".to_owned()),
        message: match found.help {
            Some(help) => format!("{}\n{help}", found.message),
            None => found.message,
        },
        ..Diagnostic::default()
    });

    found
        .into_iter()
        .map(|found| Diagnostic {
            range: convert::range_of(text, lines, found.range),
            severity: Some(match found.severity {
                phrost_analysis::Severity::Error => DiagnosticSeverity::ERROR,
                phrost_analysis::Severity::Warning => DiagnosticSeverity::WARNING,
                phrost_analysis::Severity::Note => DiagnosticSeverity::INFORMATION,
            }),
            code: Some(NumberOrString::String(found.code.to_owned())),
            source: Some("phrost".to_owned()),
            message: found.message,
            // Faded rather than underlined. The protocol keeps this separate from the severity
            // on purpose: how bad it is and what to do about it are different questions.
            // Both, because a finding may be either and the protocol carries a list. Faded for
            // what nobody reads, struck through for what is going away.
            tags: Some(
                [
                    found.unnecessary.then_some(DiagnosticTag::UNNECESSARY),
                    found.deprecated.then_some(DiagnosticTag::DEPRECATED),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            )
            .filter(|tags: &Vec<DiagnosticTag>| !tags.is_empty()),
            ..Diagnostic::default()
        })
        .chain(linted)
        .collect()
}

fn config_findings(
    workspace: &crate::workspace::Workspace,
    path: &Path,
    text: &std::sync::Arc<str>,
) -> Vec<Diagnostic> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let lines = phrost_model::LineIndex::new(text);
    let view = workspace.project_view();

    phrost_analysis::config_diagnostics(&view, name, text)
        .into_iter()
        .map(|found| Diagnostic {
            range: convert::range_of(text, &lines, found.range),
            severity: Some(match found.severity {
                phrost_analysis::Severity::Error => DiagnosticSeverity::ERROR,
                phrost_analysis::Severity::Warning => DiagnosticSeverity::WARNING,
                phrost_analysis::Severity::Note => DiagnosticSeverity::INFORMATION,
            }),
            code: Some(NumberOrString::String(found.code.to_owned())),
            source: Some("phrost".to_owned()),
            message: found.message,
            ..Diagnostic::default()
        })
        .collect()
}
