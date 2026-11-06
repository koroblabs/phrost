//! One file per kind of answer the server gives.
//!
//! The protocol methods next door stay short -- convert the request, guard it, call in here, convert
//! the answer -- and the work lives in a file of its own. Adding a request then adds a file and one
//! method, instead of another hundred lines in the one file every feature would have to edit.

mod actions;
mod calls;
mod client_word;
mod configuration;
mod describe;
mod file_rename;
mod folding;
mod formatting;
mod hierarchy;
mod highlight;
mod hints;
mod implementation;
mod lenses;
mod navigate;
mod outline;
mod positions;
mod rename;
mod selection;
mod signature;
mod tokens;
mod twig;

pub(crate) use self::tokens::legend as semantic_token_legend;

/// Where the project's configuration names the thing a reference target names.
///
/// A service's `class:` names a class as surely as a `new` does, and a route's `_controller`
/// names a method -- so a reader asking where either is used means both, and a rename that
/// rewrites only the PHP leaves a `services.yml` pointing at a class that is gone.
///
/// One function because three answers need it and one of them did not have it: find-references
/// and rename each grew their own copy of *which lookup belongs to which kind of target*, and the
/// code lens -- which says how many references there are -- grew none, so it counted the PHP and
/// called that the total. A reader who can open the list and count a different number than the
/// lens above it stops believing the lens. The ranges are the whole name as written; a caller
/// that replaces rather than points narrows them itself, which is a different question.
pub(crate) fn config_sites_for(
    workspace: &crate::workspace::Workspace,
    target: &phrost_analysis::ReferenceTarget,
) -> Vec<(std::path::PathBuf, phrost_model::TextRange)> {
    match target {
        // Both readings of a class, because a service's `class:` writes the name in full and no
        // import reaches it -- so what a search lists outside PHP and what a rename replaces there
        // are the same sites.
        phrost_analysis::ReferenceTarget::Class(fqn)
        | phrost_analysis::ReferenceTarget::ClassUnderItsOwnName(fqn) => {
            workspace.config_sites_naming(fqn.as_str())
        }
        // A route writes its controller as `Class::action`, so the action is named outside PHP
        // exactly as the class is -- 145 of them on one real project.
        phrost_analysis::ReferenceTarget::Member {
            declared_by, name, ..
        } => workspace.config_sites_calling(declared_by.as_str(), name.as_str()),
        // The same, once per class the family declares it in: a route may be written against the
        // interface or against the class implementing it, and a rename has to move both.
        phrost_analysis::ReferenceTarget::MemberFamily {
            declarers, name, ..
        } => declarers
            .iter()
            .flat_map(|declared_by| {
                workspace.config_sites_calling(declared_by.as_str(), name.as_str())
            })
            .collect(),
        // A local and an alias exist inside one file, and no configuration file names either. A
        // free function is named outside PHP only as a *string*, which the reference walk itself
        // already lists -- a container names a class and a route names `Class::action`, and
        // neither shape has a place for a bare function.
        phrost_analysis::ReferenceTarget::Function(_)
        | phrost_analysis::ReferenceTarget::Constant(_)
        | phrost_analysis::ReferenceTarget::Local { .. }
        | phrost_analysis::ReferenceTarget::Alias { .. }
        | phrost_analysis::ReferenceTarget::MagicMember { .. }
        | phrost_analysis::ReferenceTarget::ElementKey { .. }
        | phrost_analysis::ReferenceTarget::DeclaredInput { .. }
        | phrost_analysis::ReferenceTarget::TraitAlias(_)
        | phrost_analysis::ReferenceTarget::Parameter(_) => Vec::new(),
        // A key's declaration is recorded as a position rather than a range in a file read here,
        // so the reference answer adds it from the catalog -- see `catalog_declaration`.
        phrost_analysis::ReferenceTarget::CatalogKey { .. } => Vec::new(),
    }
}

/// Where a catalog key is declared, when a file the framework read declares it: a route in a
/// `*.routing.yml`, a permission in a `*.permissions.yml`.
///
/// A position and not a range, because that is how the catalog recorded it -- the file is not PHP
/// and its text is not held -- so the location is built from it directly, the way goto on the same
/// key builds its jump.
pub(crate) fn catalog_declaration(
    workspace: &crate::workspace::Workspace,
    target: &phrost_analysis::ReferenceTarget,
) -> Option<lspkit_runtime::protocol::ls_types::Location> {
    use lspkit_runtime::protocol::ls_types::{Location, Position, Range, Uri};

    let phrost_analysis::ReferenceTarget::CatalogKey { catalog, key } = target else {
        return None;
    };
    let place = workspace.project_view().catalog_place(catalog, key)?;
    let width = u32::try_from(key.chars().count()).ok()?;
    Some(Location {
        uri: Uri::from_file_path(&place.path)?,
        range: Range::new(
            Position::new(place.line, place.character),
            Position::new(place.line, place.character + width),
        ),
    })
}

impl crate::server::Backend {
    /// Every reference in one file, without the sweep.
    ///
    /// What a target that cannot leave its file needs: a local's occurrences are all in the buffer
    /// the question came from, and there is nothing to grep for.
    pub(crate) async fn references_here(
        &self,
        file: phrost_model::FileId,
        target: &phrost_analysis::ReferenceTarget,
    ) -> Vec<crate::workspace::Reference> {
        let workspace = self.workspace.read().await;
        phrost_analysis::references_in(&workspace.view(file), target)
            .into_iter()
            .map(|range| crate::workspace::Reference { file, range })
            .collect()
    }
}

/// Sweeps the project for references, one file at a time.
///
/// The loop is here rather than in the workspace because only the caller can decide how long to hold
/// the lock. Both matter and both were wrong: a single call that answered for the whole project held
/// a read lock for half a second -- which is a write lock a keystroke was waiting on -- and it could
/// not be interrupted, because an abort lands on an await and a synchronous loop has none.
///
/// So the lock is taken per file and the runtime gets a turn between them. A cancelled request stops
/// within one file of being asked to, and typing does not wait on a search.
///
/// A file list that goes slightly stale mid-sweep is the right trade: the answer is about the project
/// as it was when the question was asked.
pub(crate) async fn sweep_references(
    workspace: &tokio::sync::RwLock<crate::workspace::Workspace>,
    target: &phrost_analysis::ReferenceTarget,
    query: &phrost_analysis::SweepQuery,
) -> Vec<crate::workspace::Reference> {
    sweep_project(workspace, query, |workspace, path| {
        workspace.references_in_file(path, target)
    })
    .await
}

/// The sweep itself, for whatever question is being asked of every candidate file.
///
/// Parameterised by the question rather than written once per question: the lock discipline above
/// is the whole of what makes a project-wide request survivable, and a second copy of it is a
/// second place for it to be got wrong.
///
/// One query for a target with several spellings: a constructor is reached by writing `new Repo()`
/// and by writing `parent::__construct()`, and a file that used either has to be a candidate. All
/// of them are matched in one pass over the project, so a target with two spellings costs one read
/// of each file rather than two.
pub(crate) async fn sweep_project<T: Send>(
    workspace: &tokio::sync::RwLock<crate::workspace::Workspace>,
    query: &phrost_analysis::SweepQuery,
    per_file: impl Fn(&crate::workspace::Workspace, &std::path::Path) -> Vec<T> + Sync,
) -> Vec<T> {
    let candidates = workspace.read().await.reference_candidates(query);
    tokio::task::yield_now().await;

    let mut found = Vec::new();
    for batch in candidates.chunks(SWEEP_BATCH) {
        {
            let workspace = workspace.read().await;
            found.extend(crate::workspace::in_parallel(batch, |path| {
                per_file(&workspace, path)
            }));
        }
        // The point of the whole shape: an abort can only take effect here.
        tokio::task::yield_now().await;
    }
    found
}

/// How many candidate files a sweep answers for before letting go of the lock.
///
/// A batch rather than a file, because the answering is now spread over threads and a batch of one
/// spreads over nothing. What it trades is how soon a cancelled request stops: a file at a time was
/// the finest possible and this is a few milliseconds' worth -- against the half-second lock that
/// made the per-file shape necessary in the first place, which this is nowhere near.
const SWEEP_BATCH: usize = 64;
