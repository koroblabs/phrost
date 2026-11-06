use lspkit_fixture::Fixture;
use phrost_model::{PhpVersion, SourceOrigin};

use crate::FixtureMarkers;

/// One file described by a fixture.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FixtureFile {
    path: String,
    text: String,
    origin: SourceOrigin,
    open: bool,
    php_version: Option<PhpVersion>,
}

impl FixtureFile {
    /// Workspace-relative path, without the leading slash from the header.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// File text with all markers stripped and escapes resolved.
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn origin(&self) -> SourceOrigin {
        self.origin
    }

    /// Whether the fixture declares this file as open in the editor.
    ///
    /// Closed is the default, because that is the state a project file is normally in and the
    /// state that used to be under-tested.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Per-file language version override, if the header declared one.
    pub fn php_version(&self) -> Option<PhpVersion> {
        self.php_version
    }
}

/// A parsed fixture: its files and the marker positions stripped out of them.
#[derive(Clone, Debug)]
pub struct FixtureSpec {
    files: Vec<FixtureFile>,
    markers: FixtureMarkers,
}

impl FixtureSpec {
    /// Parses the fixture format documented at the crate root.
    ///
    /// Panics with a pointed message on malformed input: a fixture is test source, so a typo
    /// should stop the test rather than quietly produce a different project than intended.
    pub fn parse(spec: &str) -> Self {
        let (files, markers) = Fixture::parse(spec).into_parts();
        Self {
            files: files.iter().map(FixtureFile::read).collect(),
            markers: markers.map_paths(|path| relative(path).to_owned()),
        }
    }

    pub fn files(&self) -> &[FixtureFile] {
        &self.files
    }

    pub fn markers(&self) -> &FixtureMarkers {
        &self.markers
    }

    /// Splits the spec into its files and markers, for harnesses that consume both separately.
    pub fn into_parts(self) -> (Vec<FixtureFile>, FixtureMarkers) {
        (self.files, self.markers)
    }

    pub fn file(&self, path: &str) -> &FixtureFile {
        self.files
            .iter()
            .find(|file| file.path == path)
            .unwrap_or_else(|| {
                panic!(
                    "fixture has no file `{path}`; declared: {}",
                    self.files
                        .iter()
                        .map(|file| file.path.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }

    pub fn has_markers(&self) -> bool {
        !self.markers.is_empty()
    }
}

impl FixtureFile {
    /// What phrost's header flags say about one file of a parsed fixture.
    fn read(file: &lspkit_fixture::File) -> FixtureFile {
        let mut read = FixtureFile {
            path: relative(file.path()).to_owned(),
            text: file.text().to_owned(),
            origin: SourceOrigin::User,
            open: false,
            php_version: None,
        };
        for flag in file.flags() {
            match flag.as_str() {
                "vendor" => read.origin = SourceOrigin::Vendor,
                "stub" => read.origin = SourceOrigin::Stub,
                "user" => read.origin = SourceOrigin::User,
                "open" => read.open = true,
                "closed" => read.open = false,
                _ => {
                    let version = flag.strip_prefix("php:").unwrap_or_else(|| {
                        panic!("unknown fixture header flag `{flag}` on {}", file.path())
                    });
                    read.php_version = Some(version.parse().unwrap_or_else(|error| {
                        panic!("fixture header of {}: {error}", file.path())
                    }));
                }
            }
        }
        read
    }
}

/// A fixture path as every layer above names a file: relative to the root, without the slash.
fn relative(path: &str) -> &str {
    path.trim_start_matches('/')
}
