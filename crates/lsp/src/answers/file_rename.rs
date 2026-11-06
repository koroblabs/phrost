//! Moving a file, and taking the class it declares with it.
//!
//! PSR-4 makes a class's fully-qualified name a function of where its file is: the directory
//! decides the namespace and the basename decides the class. So renaming the file is a rename of
//! the class, and an editor that moves the file without saying so leaves a project that no longer
//! autoloads -- which is not a diagnostic anybody sees until something fails at runtime.
//!
//! Answered before the move rather than after (`willRenameFiles`), because the edits and the move
//! then land in one undo step and the client applies them to the file it is about to move.
//!
//! The half that is not a rewrite: a class reached by *sharing a namespace* is written bare and
//! imported nowhere, so a move breaks it and there is no line to change -- one has to be written.
//! Both directions of that, because a file has one name of its own and as many neighbours as it
//! uses: the files that reached the moved class this way, and the moved file's own reach into the
//! namespace it is leaving. Measured on a three-file project: without the two lines this writes,
//! the move it had just offered produced **four** `undefined-type` errors.

use std::collections::HashMap;

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_model::{FileId, TextSize};

use crate::{answers::positions::Positions, server::Backend};

impl Backend {
    pub(crate) async fn answer_will_rename(
        &self,
        params: RenameFilesParams,
    ) -> Result<Option<WorkspaceEdit>> {
        // Not `require_ready`: a client sends this as part of a drag, and refusing with an error
        // would put a dialogue in front of somebody who moved a file while the project was still
        // being read. Nothing to say is the right answer then.
        if !self.workspace.read().await.is_ready() {
            return Ok(None);
        }

        let mut edits: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        for rename in &params.files {
            self.plan_rename(&rename.old_uri, &rename.new_uri, &mut edits)
                .await;
        }
        if edits.is_empty() {
            return Ok(None);
        }
        Ok(Some(WorkspaceEdit {
            changes: Some(edits),
            document_changes: None,
            change_annotations: None,
        }))
    }

    /// The edits one moved file asks for, appended to what the others asked for.
    ///
    /// Silent about every reason to do nothing, and there are many honest ones: a file outside
    /// every source root, one that declares no class of its own name, one whose move changes
    /// neither half of the name. A rename that cannot be derived is not an error to report.
    async fn plan_rename(&self, old: &str, new: &str, edits: &mut HashMap<Uri, Vec<TextEdit>>) {
        let (Some(old_uri), Some(new_uri)) = (parsed(old), parsed(new)) else {
            return;
        };
        let (Some(old_path), Some(new_path)) = (old_uri.to_file_path(), new_uri.to_file_path())
        else {
            return;
        };
        let (Some(old_name), Some(new_name)) = (class_name_of(&old_path), class_name_of(&new_path))
        else {
            return;
        };

        let workspace = self.workspace.read().await;
        let Some(moved) = workspace.file_id(&old_path) else {
            return;
        };
        if !workspace.is_own_code(moved) {
            return;
        }
        // The first is the most specific rule, which is the namespace the file is expected to
        // declare -- see `permitted_namespaces` for why there can be more than one.
        let (Some(old_namespace), Some(new_namespace)) = (
            workspace.permitted_namespace(&old_path),
            workspace.permitted_namespace(&new_path),
        ) else {
            return;
        };
        if old_namespace == new_namespace && old_name == new_name {
            return;
        }

        // The declaration this file is named after, and where the file says which namespace it is
        // in. Both come from the tree: the class is found by name among what the file declares, so
        // a file holding several classes contributes only the one PSR-4 can reach.
        let Some(Some((declaration, namespace))) = workspace.analysed(moved, |view| {
            let declaration = phrost_analysis::document_symbols(view)
                .into_iter()
                .find(|entry| {
                    entry.label == old_name && matches!(entry.kind, phrost_model::SymbolKind::Class)
                })
                .map(|entry| entry.selection_range);
            Some((declaration?, view.declared_namespace()))
        }) else {
            return;
        };
        let target = workspace.analysed(moved, |view| {
            phrost_analysis::rename_target(view, declaration.start())
                .map(|target| (target.sweep_query(view), target))
        });
        let Some(Some((needles, target))) = target else {
            return;
        };
        drop(workspace);

        if needles.is_empty() {
            return;
        }
        let found = super::sweep_references(&self.workspace, &target, &needles).await;

        let workspace = self.workspace.read().await;
        let mut positions = Positions::over(&workspace);

        // The file's own `namespace` line, when the move changed which namespace it is in. Skipped
        // for a file with none or with several: `declared_namespace` says why neither is a file a
        // path can name.
        if old_namespace != new_namespace
            && let Some(namespace) = namespace
            && let Some(uri) = positions.uri(moved)
            && let Some(at) = positions.range(moved, namespace.name_range)
        {
            edits.entry(uri).or_default().push(TextEdit {
                range: at,
                new_text: new_namespace.clone(),
            });
        }

        let new_fqn = format!("{new_namespace}\\{new_name}");
        let old_fqn = format!("{old_namespace}\\{old_name}");
        // Where a bare mention of the class is written, per file. Bare is what a namespace change
        // breaks and what an import fixes: it resolved either through a `use` line, which this
        // pass rewrites, or through sharing a namespace with the class, which stops being true.
        let mut bare: HashMap<FileId, TextSize> = HashMap::new();
        for reference in &found {
            if let Some(written) = positions.written(reference.file, reference.range)
                && !written.trim_start_matches('\\').contains('\\')
            {
                bare.entry(reference.file)
                    .or_insert(reference.range.start());
            }
        }

        for reference in found {
            let Some(uri) = positions.uri(reference.file) else {
                continue;
            };
            let Some(written) = positions.written(reference.file, reference.range) else {
                continue;
            };
            // Three cases, and the middle one is why this cannot be one rule. A name written
            // *qualified* while the namespace moves has to be rewritten whole -- `use App\Repo;`
            // names a place that no longer holds the class. A name written qualified while only
            // the basename changes needs its last segment and no more, which is the edit an
            // ordinary rename makes and keeps the diff to the word that changed. And a *bare*
            // name is never rewritten whole whatever moved: it leans on an import or on sharing a
            // namespace, and a qualified name written there would resolve relative to wherever it
            // sits.
            let qualified = written.trim_start_matches('\\').contains('\\');
            let edit = match (qualified, old_namespace != new_namespace) {
                (true, true) => {
                    let leading = match written.starts_with('\\') {
                        true => "\\",
                        false => "",
                    };
                    positions
                        .range(reference.file, reference.range)
                        .map(|at| (at, format!("{leading}{new_fqn}")))
                }
                _ if old_name != new_name => positions
                    .rename_range(reference.file, reference.range)
                    .map(|at| (at, new_name.clone())),
                _ => None,
            };
            if let Some((range, new_text)) = edit {
                edits
                    .entry(uri)
                    .or_default()
                    .push(TextEdit { range, new_text });
            }
        }

        // Everything above rewrites names somebody already wrote. What is left is the ones they
        // did not: a class reached by sharing a namespace is written bare and imported nowhere,
        // and moving one of the two out is what turns a rename into a bug somebody else finds.
        if old_namespace != new_namespace {
            let mut wanted: Vec<Wanted> = bare
                .into_iter()
                .filter(|(file, _)| *file != moved)
                .map(|(file, at)| Wanted {
                    file,
                    at,
                    write: new_fqn.clone(),
                    instead: old_fqn.clone(),
                })
                .collect();
            wanted.extend(orphans_of(&workspace, moved, &old_namespace, &old_fqn));
            add_imports(&workspace, &mut positions, wanted, edits);
        }
    }
}

/// The names the *moved* file reached by sharing a namespace with them.
///
/// The mirror of the case above and the larger one: a file has one name of its own and as many
/// neighbours as it uses, and every one of those written bare stops resolving the moment the file
/// leaves. Read from what the file writes rather than from a sweep, because the question is about
/// this file's names and not about any one class's uses.
fn orphans_of(
    workspace: &crate::workspace::Workspace,
    moved: FileId,
    old_namespace: &str,
    old_fqn: &str,
) -> Vec<Wanted> {
    workspace
        .analysed(moved, |view| {
            // Every class the moved file declares, not only the one it is named after. A PHP file
            // may declare several -- a test file with its fixtures is the ordinary case -- and all
            // of them move with it, so a bare name reaching one needs no import. Written against
            // `old_fqn` alone, this offered `use Illuminate\Tests\Queue\BaseNotification;` for a
            // class declared 190 lines further down the same file, and PHP refuses it: `Cannot use
            // ... as BaseNotification because the name is already in use`. The file stops loading,
            // which is the worst thing a move can do and the one thing `php -l` sees.
            let travelling = view
                .declared_classes()
                .into_iter()
                .map(|declared| declared.fqn.as_str().to_owned())
                .collect::<std::collections::HashSet<_>>();

            view.name_references()
                .into_iter()
                .filter(|name| !name.path.as_str().contains('\\'))
                .filter_map(|name| {
                    let resolved = name.resolved.as_ref()?.as_str();
                    // Resolved by sharing the namespace, which is the only way a bare name breaks
                    // here. One reached through a `use` line keeps working, because the line moves
                    // with the file; one reached anywhere else never depended on this.
                    let neighbour = format!("{old_namespace}\\{}", name.path.as_str());
                    (resolved == neighbour && resolved != old_fqn && !travelling.contains(resolved))
                        .then(|| Wanted {
                            file: moved,
                            at: name.range.start(),
                            instead: neighbour.clone(),
                            write: neighbour,
                        })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One `use` line this pass may have to write.
struct Wanted {
    file: FileId,
    /// Where the bare name is, which is what decides the namespace block to write into.
    at: TextSize,
    /// The name to import.
    write: String,
    /// A second spelling that counts as already imported.
    ///
    /// The file that referenced the moved class usually has `use <old>;` and this pass is
    /// rewriting that line to `<new>` -- so asking only whether it imports the new name answers
    /// no, and the file gets the import twice. The two are one question with two spellings, which
    /// is the shape §8 keeps recording.
    instead: String,
}

/// Adds a `use` line per name that needs one, at most one per name per file.
///
/// Grouped and joined rather than emitted one at a time, because every insertion into a file
/// lands at the same offset -- the end of its last import -- and two text edits sharing a
/// position is a thing the protocol leaves the client to order.
fn add_imports(
    workspace: &crate::workspace::Workspace,
    positions: &mut Positions<'_>,
    wanted: Vec<Wanted>,
    edits: &mut HashMap<Uri, Vec<TextEdit>>,
) {
    let mut by_file: HashMap<FileId, Vec<Wanted>> = HashMap::new();
    for one in wanted {
        by_file.entry(one.file).or_default().push(one);
    }

    for (file, mut names) in by_file {
        names.sort_by(|left, right| (left.at, &left.write).cmp(&(right.at, &right.write)));
        names.dedup_by(|left, right| left.write == right.write);
        let placed = workspace.analysed(file, |view| {
            let imported = |fqn: &str| view.imports_path(fqn, phrost_view::ImportKind::ClassLike);
            names
                .iter()
                .filter(|one| !imported(&one.write) && !imported(&one.instead))
                .filter_map(|one| {
                    view.import_placement(one.at, &one.write, phrost_view::ImportKind::ClassLike)
                })
                .map(|edit| (edit.at, edit.text))
                .collect::<Vec<_>>()
        });
        let (Some(placed), Some(uri)) = (placed, positions.uri(file)) else {
            continue;
        };
        // One edit per insertion point, which for a file that has imports is one for all of them.
        let mut joined: std::collections::BTreeMap<TextSize, String> = Default::default();
        for (at, text) in placed {
            joined.entry(at).or_default().push_str(&text);
        }
        for (at, new_text) in joined {
            if let Some(range) = positions.range(file, phrost_model::TextRange::empty(at)) {
                edits
                    .entry(uri.clone())
                    .or_default()
                    .push(TextEdit { range, new_text });
            }
        }
    }
}

/// The class a PSR-4 file is named after: its basename, when it is PHP.
fn class_name_of(path: &std::path::Path) -> Option<String> {
    match path.extension()?.to_str()? {
        "php" => Some(path.file_stem()?.to_str()?.to_owned()),
        _ => None,
    }
}

/// A URI the client sent as a string, which is how the file-operation requests carry them.
fn parsed(uri: &str) -> Option<Uri> {
    uri.parse().ok()
}
