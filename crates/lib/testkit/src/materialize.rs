use std::{
    fs,
    path::{Path, PathBuf},
};

use tempfile::TempDir;

use crate::spec::{FixtureFile, FixtureSpec};

/// A fixture written to a temporary directory on disk.
///
/// Most tests never need this: layers above the VFS work from text, and keeping them off the
/// filesystem is what makes them fast. It exists for the parts that genuinely read files —
/// composer autoload discovery, workspace scanning, disk-cache round trips — because faking those
/// is how a test ends up passing against a project layout that could not exist.
#[derive(Debug)]
pub struct MaterializedFixture {
    root: TempDir,
    spec: FixtureSpec,
}

impl MaterializedFixture {
    pub fn new(spec: FixtureSpec) -> Self {
        let root = tempfile::Builder::new()
            .prefix("phrost-fixture-")
            .tempdir()
            .expect("fixture root directory should be created");
        let fixture = Self { root, spec };

        for file in fixture.spec.files() {
            fixture.write(file.path(), file.text());
        }

        fixture
    }

    pub fn parse(spec: &str) -> Self {
        Self::new(FixtureSpec::parse(spec))
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn spec(&self) -> &FixtureSpec {
        &self.spec
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.path().join(relative)
    }

    /// Writes more files into the existing root and returns what was written.
    ///
    /// Deliberately just filesystem materialization: whether this counts as a save, an external
    /// change, or a generated file is the caller's interpretation, not this crate's.
    pub fn write_more(&self, spec: &str) -> Vec<FixtureFile> {
        let parsed = FixtureSpec::parse(spec);
        for file in parsed.files() {
            self.write(file.path(), file.text());
        }

        parsed.into_parts().0
    }

    /// Removes a file, for tests that exercise deletion and invalidation.
    pub fn remove(&self, relative: &str) {
        fs::remove_file(self.path(relative)).expect("fixture file should be removable");
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture directories should be created");
        }
        fs::write(path, text).expect("fixture file should be written");
    }
}
