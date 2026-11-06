use std::path::{Path, PathBuf};

use phrost_model::SourceOrigin;

/// A project's root directory and where its vendor code lives.
///
/// Deliberately the whole of it: no docroot detection, no framework detection, no file
/// discovery. Those need to look at what is actually on disk and decide *which* files matter for
/// a given setup, which is exactly the kind of judgement call that belongs in a later phase, not
/// baked into the one type everything else about a project depends on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProjectLayout {
    root: PathBuf,
    vendor_dir: PathBuf,
}

impl ProjectLayout {
    /// A layout with composer's own default vendor directory, `<root>/vendor`.
    ///
    /// Composer lets a project override this via `config.vendor-dir` in `composer.json`; reading
    /// that is the caller's job (it already has to parse that file for other reasons) — use
    /// [`Self::with_vendor_dir`] once it knows the real answer.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        // Absolute on the way in, because a file's stored path is kept *relative to the root* and
        // the root is also what the disk cache is keyed by. A root spelled `.` stores
        // `./docroot/x.php` where a server stored `docroot/x.php`, so a warm start matched 137 rows
        // of 30 965 -- the 137 being the standard library, which lives outside any project -- and
        // called itself warm while doing the cold one's work. An editor always sends an absolute
        // `rootUri`; the CLI is the only client that can pass on whatever the shell had.
        //
        // `absolute` and not `canonicalize`, and the difference is load-bearing. Canonicalising
        // resolves symlinks, which moves an already-absolute root out from under every path the
        // *client* sends afterwards: tried, and it hung a flow test, because macOS resolves a
        // temporary directory's `/var` to `/private/var` and the watcher stopped recognising
        // notifications about its own project, so the re-index never ran. This one is lexical and
        // leaves an absolute path exactly as it was. Only a key may canonicalise -- see
        // [`Self::identity`].
        let root = root.into();
        let root = std::path::absolute(&root).unwrap_or(root);
        let vendor_dir = root.join("vendor");
        Self { root, vendor_dir }
    }

    pub fn with_vendor_dir(root: impl Into<PathBuf>, vendor_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            vendor_dir: vendor_dir.into(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The project's canonical root, for anything that needs an *identity* rather than a path.
    ///
    /// Two spellings of one directory are one project, and only a key has to know that.
    /// `phrost index --warm .` reused **0** of 30 965 rows where the same command spelled
    /// absolutely reused all of them: the disk cache is keyed by a digest of the root, and `.`
    /// digests to somewhere no server ever wrote. Nothing fails -- the warm start does the cold
    /// one's work and calls itself warm while doing it, which makes the instrument that asks
    /// whether the cache earns its 45 MB answer no.
    ///
    /// Deliberately *not* what [`Self::root`] returns, and the difference is load-bearing.
    /// Canonicalising the root itself was tried and hung a flow test: every other use of the root
    /// is a comparison against a path the **client** spelled -- a watched-file notification, an
    /// opened document -- and macOS resolves a temporary directory's `/var` to `/private/var`, so
    /// the watcher stopped recognising its own project and the re-index never ran. An identity is
    /// wanted by keys and by nothing else.
    ///
    /// Canonical rather than merely absolute, because a symlinked checkout is the same project.
    /// A root that is not on disk has no identity to find and keeps the spelling it was given.
    pub fn identity(&self) -> PathBuf {
        std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone())
    }

    pub fn vendor_dir(&self) -> &Path {
        &self.vendor_dir
    }

    /// Whether `path` lives inside the vendor directory.
    ///
    /// This is the whole of what decides [`SourceOrigin`] for a file: everything else about
    /// "does this deserve full analysis" follows from that one classification, and it must never
    /// depend on anything more elaborate than the paths a project already told us about.
    pub fn is_vendor_path(&self, path: &Path) -> bool {
        path.starts_with(&self.vendor_dir)
    }

    pub fn source_origin(&self, path: &Path) -> SourceOrigin {
        if self.is_vendor_path(path) {
            SourceOrigin::Vendor
        } else {
            SourceOrigin::User
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A root arrives as whatever the shell had, and everything downstream needs it absolute.
    #[test]
    fn a_relative_root_is_resolved_against_the_working_directory() {
        let layout = ProjectLayout::new("some/project");

        assert!(layout.root().is_absolute(), "got {:?}", layout.root());
        assert!(layout.root().ends_with("some/project"));
    }

    /// An absolute root is left exactly as given, symlinks and all, because every path the client
    /// sends afterwards is compared against it.
    #[test]
    fn an_absolute_root_is_left_alone() {
        let home = tempfile::tempdir().expect("a temporary directory");
        // On macOS this is under `/var`, which is a symlink to `/private/var` -- the spelling that
        // canonicalising would have silently substituted.
        assert_eq!(ProjectLayout::new(home.path()).root(), home.path());
    }

    /// The property the disk cache rests on: one directory, however it is written, is one project.
    #[test]
    fn two_spellings_of_one_directory_share_an_identity() {
        let home = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir(home.path().join("src")).expect("a directory to step through");

        let direct = ProjectLayout::new(home.path());
        let roundabout = ProjectLayout::new(home.path().join("src").join(".."));

        assert_eq!(direct.identity(), roundabout.identity());
        // And the root each was given is left alone, because everything but a key compares it
        // against a path somebody else spelled.
        assert_ne!(direct.root(), roundabout.root());
    }

    /// A root that is not on disk keeps the spelling it was given, so a caller over a path that
    /// does not exist yet gets an identity rather than a failure.
    #[test]
    fn a_root_that_cannot_be_resolved_is_its_own_identity() {
        let layout = ProjectLayout::new("/no/such/project");
        assert_eq!(layout.identity(), Path::new("/no/such/project"));
    }

    #[test]
    fn defaults_the_vendor_directory_to_root_slash_vendor() {
        let layout = ProjectLayout::new("/proj");
        assert_eq!(layout.root(), Path::new("/proj"));
        assert_eq!(layout.vendor_dir(), Path::new("/proj/vendor"));
    }

    #[test]
    fn accepts_a_vendor_directory_outside_the_default_layout() {
        // `config.vendor-dir` in composer.json can point anywhere, including outside `root/`
        // entirely (a shared vendor cache in a monorepo, for instance).
        let layout = ProjectLayout::with_vendor_dir("/proj", "/shared/vendor-cache");
        assert_eq!(layout.vendor_dir(), Path::new("/shared/vendor-cache"));
    }

    #[test]
    fn classifies_paths_inside_and_outside_vendor() {
        let layout = ProjectLayout::new("/proj");

        assert!(layout.is_vendor_path(Path::new("/proj/vendor/psr/log/Logger.php")));
        assert!(!layout.is_vendor_path(Path::new("/proj/src/Repo.php")));

        // A sibling directory that merely starts with the same characters ("vendor2") must not
        // be classified as vendor by a naive string-prefix check. `Path::starts_with` compares
        // whole components, so this is correct without any extra guard — but it is exactly the
        // kind of thing a future refactor could break by swapping in a string comparison.
        assert!(!layout.is_vendor_path(Path::new("/proj/vendor2/Thing.php")));
    }

    #[test]
    fn source_origin_follows_directly_from_vendor_membership() {
        let layout = ProjectLayout::new("/proj");
        assert_eq!(
            layout.source_origin(Path::new("/proj/vendor/psr/log/Logger.php")),
            SourceOrigin::Vendor
        );
        assert_eq!(
            layout.source_origin(Path::new("/proj/src/Repo.php")),
            SourceOrigin::User
        );
    }
}
