//! The PHP standard library, as declaration-only source the index can read.
//!
//! `strlen`, `\Exception`, `ArrayAccess`, `DateTimeInterface` — everything that exists in PHP
//! without being declared in any project file. Nothing above the index can invent them, and there
//! is no fallback that would paper over their absence: an unresolvable name returns *not found*,
//! so without stubs the whole standard library is simply invisible to goto, hover, and completion.
//!
//! # Why they are written to disk rather than kept in memory
//!
//! Because a docblock is read on demand from a range in a file. That is what keeps the index from
//! holding tens of megabytes of vendor documentation, and it means a declaration without a readable
//! path has no documentation at all. Extracting to a cache directory also means goto into `strlen`
//! opens a real file in any editor, instead of a custom URI scheme each client would have to
//! support.
//!
//! # Why the directory is content-addressed
//!
//! The cache path contains a hash of the stub set. An upgrade writes a new directory rather than
//! rewriting the old one, so a half-finished extraction can never be mistaken for a complete one,
//! and two versions of phrost on one machine do not fight.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

include!(concat!(env!("OUT_DIR"), "/prelude_table.rs"));

#[cfg(test)]
mod tests;

/// Where the standard library was materialized, and what it contains.
///
/// Empty is a legitimate value — [`Self::empty`] — for tests and for anyone who wants an index of
/// nothing but project code.
#[derive(Clone, Debug, Default)]
pub struct StubSet {
    root: Option<PathBuf>,
    files: Vec<PathBuf>,
}

impl StubSet {
    /// No standard library at all. What tests use, so they never touch a real cache directory.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Extracts the embedded stubs under `cache_root`, reusing them if they are already there.
    ///
    /// The check is a stamp file holding the set's hash, not a per-file comparison: rereading 1.5 MB
    /// on every startup to confirm nothing changed is work with a known answer.
    pub fn install_in(cache_root: &Path) -> io::Result<Self> {
        let root = cache_root.join(Self::digest());
        let stamp = root.join(".complete");

        if !stamp.exists() {
            fs::create_dir_all(&root)?;
            for (name, contents) in PRELUDE {
                fs::write(root.join(name), contents)?;
            }
            // Written last, so an interrupted extraction leaves the directory unstamped and the
            // next run redoes it rather than trusting half a standard library.
            fs::write(&stamp, Self::digest())?;
        }

        Ok(Self {
            files: PRELUDE.iter().map(|(name, _)| root.join(name)).collect(),
            root: Some(root),
        })
    }

    /// The same, under this machine's cache directory.
    pub fn install() -> io::Result<Self> {
        let cache = dirs::cache_dir()
            .ok_or_else(|| io::Error::other("this platform has no cache directory"))?;
        Self::install_in(&cache.join("phrost").join("stubs"))
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// A hash of every stub's name and contents.
    ///
    /// Names included as well as contents, because a file being *added* changes what the index
    /// knows even when no existing file changed.
    fn digest() -> String {
        let mut hasher = blake3::Hasher::new();
        for (name, contents) in PRELUDE {
            hasher.update(name.as_bytes());
            hasher.update(contents.as_bytes());
        }
        hasher.finalize().to_hex()[..16].to_owned()
    }
}
