use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use phrost_model::{FileId, SourceOrigin};
use rustc_hash::FxHashMap;

use crate::{
    document::{DocumentVersion, OpenDocument},
    source::SourceText,
};

/// File identity for one session, plus the text of whatever is open.
///
/// Registration is append-only: a [`FileId`] handed out once keeps meaning the same file for the
/// life of the session, even after the file is deleted on disk. Reusing ids would let a stale
/// index row silently point at an unrelated file.
#[derive(Debug, Default)]
pub struct Vfs {
    files: Vec<FileEntry>,
    by_path: FxHashMap<Arc<Path>, FileId>,
    open: FxHashMap<FileId, OpenDocument>,
}

#[derive(Debug)]
struct FileEntry {
    path: Arc<Path>,
    origin: SourceOrigin,
}

impl Vfs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the id for `path`, registering it on first sight.
    ///
    /// Re-registering an existing path with a different origin updates the origin: a file can
    /// move between user code and vendor when composer configuration changes, and the id must
    /// survive that so open documents and index rows keyed by it stay valid.
    pub fn register(&mut self, path: impl AsRef<Path>, origin: SourceOrigin) -> FileId {
        let path = path.as_ref();
        if let Some(&existing) = self.by_path.get(path) {
            self.files[existing.as_raw() as usize].origin = origin;
            return existing;
        }

        let path: Arc<Path> = Arc::from(path.to_path_buf());
        let id = FileId::from_raw(u32::try_from(self.files.len()).expect("file count fits in u32"));
        self.files.push(FileEntry {
            path: Arc::clone(&path),
            origin,
        });
        self.by_path.insert(path, id);
        id
    }

    pub fn file_id(&self, path: impl AsRef<Path>) -> Option<FileId> {
        self.by_path.get(path.as_ref()).copied()
    }

    pub fn path(&self, file: FileId) -> &Path {
        &self.entry(file).path
    }

    pub fn origin(&self, file: FileId) -> SourceOrigin {
        self.entry(file).origin
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Every registered file, in registration order.
    pub fn files(&self) -> impl ExactSizeIterator<Item = FileId> {
        (0..u32::try_from(self.files.len()).expect("file count fits in u32")).map(FileId::from_raw)
    }

    // --- open documents ---

    /// Records the text of an open buffer, replacing any previous revision.
    pub fn open(&mut self, file: FileId, source: SourceText, version: DocumentVersion) {
        self.open.insert(file, OpenDocument::new(source, version));
    }

    /// Drops the open buffer for `file`.
    ///
    /// Nothing else happens here. Closing a file must not trigger a disk read or re-analysis: the
    /// index already holds this file's skeleton, so close is pure forgetting.
    pub fn close(&mut self, file: FileId) -> Option<OpenDocument> {
        self.open.remove(&file)
    }

    pub fn open_document(&self, file: FileId) -> Option<&OpenDocument> {
        self.open.get(&file)
    }

    pub fn is_open(&self, file: FileId) -> bool {
        self.open.contains_key(&file)
    }

    pub fn open_count(&self) -> usize {
        self.open.len()
    }

    /// The text to analyze for `file`: the open buffer if there is one, otherwise disk.
    ///
    /// Callers that must distinguish the two — anything returning a source location that a later
    /// edit could invalidate — should ask [`Self::is_open`] instead of using this.
    pub fn source(&self, file: FileId) -> io::Result<SourceText> {
        match self.open.get(&file) {
            Some(document) => Ok(document.source().clone()),
            None => SourceText::read(self.path(file)),
        }
    }

    fn entry(&self, file: FileId) -> &FileEntry {
        self.files
            .get(file.as_raw() as usize)
            .unwrap_or_else(|| panic!("{file:?} was never registered in this Vfs"))
    }
}

/// Normalizes a path for registration: absolute, with `.` and `..` resolved lexically.
///
/// Lexical rather than `canonicalize`, so it works for files that do not exist yet — a file the
/// editor has open but has never saved still needs a stable identity. Symlinks are therefore not
/// resolved, which is the right call for a project: `vendor/` symlinked into a monorepo should
/// keep the path the user sees.
pub fn normalize(base: &Path, path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };

    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_paths_round_trip() {
        let mut vfs = Vfs::new();
        let repo = vfs.register("/p/src/Repo.php", SourceOrigin::User);
        let logger = vfs.register("/p/vendor/psr/log/Logger.php", SourceOrigin::Vendor);

        assert_ne!(repo, logger);
        assert_eq!(vfs.register("/p/src/Repo.php", SourceOrigin::User), repo);
        assert_eq!(vfs.file_id("/p/src/Repo.php"), Some(repo));
        assert_eq!(vfs.path(logger), Path::new("/p/vendor/psr/log/Logger.php"));
        assert_eq!(vfs.origin(logger), SourceOrigin::Vendor);
        assert_eq!(vfs.len(), 2);
    }

    #[test]
    fn re_registering_moves_a_file_between_origins_without_changing_its_id() {
        // Composer config can change what counts as vendor. The id has to survive that, or every
        // index row and open document keyed by it silently points somewhere else.
        let mut vfs = Vfs::new();
        let file = vfs.register("/p/packages/local/src/A.php", SourceOrigin::Vendor);
        assert_eq!(vfs.origin(file), SourceOrigin::Vendor);

        assert_eq!(
            vfs.register("/p/packages/local/src/A.php", SourceOrigin::User),
            file
        );
        assert_eq!(vfs.origin(file), SourceOrigin::User);
    }

    #[test]
    fn open_and_close_only_touch_the_open_set() {
        let mut vfs = Vfs::new();
        let file = vfs.register("/p/src/A.php", SourceOrigin::User);
        assert!(!vfs.is_open(file));

        vfs.open(
            file,
            SourceText::new("<?php class A {}"),
            DocumentVersion::new(0),
        );
        assert!(vfs.is_open(file));
        assert_eq!(
            vfs.open_document(file).map(|doc| doc.version().as_i32()),
            Some(0)
        );

        // Version 0 is a real version: some editors open documents at 0, and treating it as
        // "unset" makes those documents lose full analysis.
        vfs.open(
            file,
            SourceText::new("<?php class B {}"),
            DocumentVersion::new(1),
        );
        assert_eq!(
            vfs.open_document(file).map(|doc| doc.source().text()),
            Some("<?php class B {}")
        );

        assert!(vfs.close(file).is_some());
        assert!(!vfs.is_open(file));
        // The file itself is still registered; only its buffer is gone.
        assert_eq!(vfs.file_id("/p/src/A.php"), Some(file));
    }

    #[test]
    fn invalid_utf8_is_replaced_rather_than_rejected() {
        // A stray Latin-1 byte in a vendor file must not make the whole file unreadable; every
        // identifier in it is still ASCII and still needed.
        let source = SourceText::from_bytes(b"<?php // \xa9 copyright\nclass A {}");
        assert!(source.text().contains("class A {}"));
        assert!(source.text().contains('\u{fffd}'));
    }

    #[test]
    fn stamps_follow_content_not_length() {
        let a = SourceText::new("<?php class A {}");
        let b = SourceText::new("<?php class B {}");
        assert_eq!(a.len(), b.len());
        assert_ne!(a.stamp(), b.stamp());
        assert_eq!(a.stamp(), SourceText::new("<?php class A {}").stamp());
    }

    #[test]
    fn normalize_resolves_dots_lexically_against_a_base() {
        let base = Path::new("/p/web/core");
        assert_eq!(
            normalize(base, Path::new("../../vendor/./autoload.php")),
            PathBuf::from("/p/vendor/autoload.php")
        );
        assert_eq!(
            normalize(base, Path::new("/absolute/x.php")),
            PathBuf::from("/absolute/x.php")
        );
    }
}
