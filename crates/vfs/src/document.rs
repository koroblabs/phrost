use crate::source::SourceText;

/// Editor-supplied revision number for an open document.
///
/// Signed and permitted to be zero, because that is what clients actually send: some editors open
/// a document at version 0, and treating 0 as "unset" makes those documents look like background
/// files and quietly lose full analysis.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocumentVersion(i32);

impl DocumentVersion {
    pub const fn new(version: i32) -> Self {
        Self(version)
    }

    pub const fn as_i32(self) -> i32 {
        self.0
    }
}

/// One open editor buffer: the exact text the user is looking at, and which revision it is.
///
/// Whether this text differs from disk is the caller's question to ask. This type only promises
/// that its text and version belong together.
#[derive(Clone, Debug)]
pub struct OpenDocument {
    source: SourceText,
    version: DocumentVersion,
}

impl OpenDocument {
    pub fn new(source: SourceText, version: DocumentVersion) -> Self {
        Self { source, version }
    }

    pub fn source(&self) -> &SourceText {
        &self.source
    }

    pub fn version(&self) -> DocumentVersion {
        self.version
    }
}
