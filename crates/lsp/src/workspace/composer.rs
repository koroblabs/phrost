//! Telling composer's files apart, and telling a composer run that is over from one still going.
//!
//! Composer writes `composer.lock` *first*, then replaces the packages, then records the install in
//! `<vendor>/composer/installed.json`, and rewrites an autoload file only when its content changed.
//! So the lock is the one file that changes at the start of a run, and the record the one that
//! changes at the end of every run that installed anything. A server that reads the project when the
//! lock changes reads a vendor tree half replaced -- and an editor that does not walk an ignored
//! `vendor/` never reports the record, so nothing told it to read again.

use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::Workspace;

/// The files composer writes into its own directory under `vendor/` that are worth hearing about:
/// the record of what is installed, which changes at the end of every install, and the generated
/// autoload map, which decides where code lives.
pub(crate) const COMPOSER_RECORDS: &[&str] = &["installed.json", "autoload_*.php"];

/// When the lock and the install record were last written.
///
/// Compared rather than read for content: composer writes the two in a fixed order, and which is
/// newer is the whole of what says whether a run has finished.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ComposerStamp {
    locked: Option<SystemTime>,
    installed: Option<SystemTime>,
}

impl ComposerStamp {
    /// Whether a lock has been written that no install has followed yet.
    ///
    /// A lock with no record at all counts: it is an install still to come, and a tree read before
    /// it is a tree with no dependencies in it. No lock is never installing -- a project without one
    /// has nothing for composer to be in the middle of.
    pub fn installing(&self) -> bool {
        match (self.locked, self.installed) {
            (Some(locked), Some(installed)) => locked > installed,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }
}

impl Workspace {
    /// The directory composer keeps its install record and its autoload map in.
    pub fn composer_records(&self) -> Option<PathBuf> {
        Some(self.layout.as_ref()?.vendor_dir().join("composer"))
    }

    /// When the lock and the install record were last written, read from the disk now.
    pub fn composer_stamp(&self) -> ComposerStamp {
        let Some(layout) = self.layout.as_ref() else {
            return ComposerStamp::default();
        };
        let written = |path: PathBuf| {
            std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
        };
        ComposerStamp {
            locked: written(layout.root().join("composer.lock")),
            installed: written(layout.vendor_dir().join("composer").join("installed.json")),
        }
    }

    /// Whether a path is the project's dependency directory itself.
    ///
    /// Composer's first write in a checkout that was never installed, and its first act in one
    /// whose dependencies were thrown away: neither touches the lock, and the directory the install
    /// record goes in does not exist for anything to watch. The directory's own entry does, in the
    /// root every client keeps -- so it appearing or going is a composer run like any other, waited
    /// out the same way.
    pub(super) fn is_dependency_directory(&self, path: &Path) -> bool {
        self.layout
            .as_ref()
            .is_some_and(|layout| layout.vendor_dir() == path)
    }

    /// Whether a path is one of composer's own: its configuration, its lock, or a record it writes
    /// beside the packages.
    ///
    /// The records are recognised by the directory they sit in rather than by the project's vendor
    /// path, the way they always were: a client that cannot root a watch reports them by a glob
    /// that matches any `composer/` directory, and the rule has to agree with what it was sent.
    pub(super) fn is_composer_file(path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if matches!(name, "composer.json" | "composer.lock") {
            return true;
        }
        let recorded =
            name == "installed.json" || (name.starts_with("autoload_") && name.ends_with(".php"));
        recorded
            && path
                .parent()
                .is_some_and(|parent| parent.ends_with("composer"))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ComposerStamp;

    fn at(seconds: u64) -> Option<std::time::SystemTime> {
        Some(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
    }

    #[test]
    fn a_run_is_over_when_the_record_is_no_older_than_the_lock() {
        let reading = |locked, installed| ComposerStamp { locked, installed }.installing();
        assert!(reading(at(20), at(10)), "a lock nothing has installed yet");
        assert!(
            !reading(at(10), at(20)),
            "an install recorded after its lock"
        );
        assert!(!reading(at(10), at(10)), "written in the same instant");
        assert!(
            reading(at(10), None),
            "a lock with no install ever recorded"
        );
        assert!(!reading(None, at(10)), "no lock, nothing to wait for");
        assert!(!reading(None, None));
    }
}
