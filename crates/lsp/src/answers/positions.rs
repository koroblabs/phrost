//! Turning stored byte ranges into client positions, across however many files an answer spans.
//!
//! A range is an offset into a file's bytes; a client wants a line and a UTF-16 column. Converting
//! needs that file's text, and an answer like "everywhere this class is used" spans a hundred files
//! with several ranges in each -- so each file is read once and its line map kept for the rest of
//! the answer.
//!
//! One place, because three answers needed it and three copies of a cache is three chances to read
//! the same file a hundred times.

use std::{collections::HashMap, sync::Arc};

use lspkit_runtime::protocol::ls_types::{Location, Range, Uri};
use phrost_model::{FileId, LineIndex, TextRange};

use crate::{convert, workspace::Workspace};

pub(crate) struct Positions<'a> {
    workspace: &'a Workspace,
    read: HashMap<FileId, (Arc<str>, LineIndex)>,
    /// The same, for the files that have no id: see [`Self::locate_path`].
    read_paths: HashMap<std::path::PathBuf, (Arc<str>, LineIndex)>,
}

impl<'a> Positions<'a> {
    pub(crate) fn over(workspace: &'a Workspace) -> Self {
        Self {
            workspace,
            read: HashMap::new(),
            read_paths: HashMap::new(),
        }
    }

    /// The client range for a byte range in one file.
    ///
    /// `None` when the file cannot be read, which is a file that vanished between being indexed and
    /// being reported -- one answer to leave out rather than a reason to fail the request.
    pub(crate) fn range(&mut self, file: FileId, range: TextRange) -> Option<Range> {
        let (text, lines) = self.text_of(file)?;
        Some(convert::range_of(text, lines, range))
    }

    /// The same, as a location a client can jump to.
    pub(crate) fn locate(&mut self, file: FileId, range: TextRange) -> Option<Location> {
        let uri = Uri::from_file_path(self.workspace.path_of(file))?;
        Some(Location {
            uri,
            range: self.range(file, range)?,
        })
    }

    /// The client range a rename replaces: the last segment of what is written there.
    ///
    /// Needs the file's text, which is the reason it lives here rather than beside the rename: the
    /// decision is `phrost-analysis`', and this only reads the bytes it needs to make it.
    pub(crate) fn rename_range(&mut self, file: FileId, range: TextRange) -> Option<Range> {
        let (text, lines) = self.text_of(file)?;
        let at = phrost_analysis::rename_range(text, range);
        Some(convert::range_of(text, lines, at))
    }

    /// What is actually written at a range, which decides how a name may be replaced.
    ///
    /// A reference to a class is written two ways and they take opposite edits: `App\\Repo` names
    /// the class outright and can be replaced whole, while a bare `Repo` leans on an import and
    /// replacing it with a qualified name would resolve *relative* to the enclosing namespace.
    pub(crate) fn written(&mut self, file: FileId, range: TextRange) -> Option<&str> {
        let (text, _) = self.text_of(file)?;
        text.get(usize::from(range.start())..usize::from(range.end()))
    }

    /// The same, for a file the index does not hold.
    ///
    /// A configuration file has no `FileId` -- it is read for what it *says* about PHP and holds
    /// no declarations of its own -- and a reference written in one still has to be pointed at.
    /// Its own small cache, because one answer can name the same file several times and reading a
    /// service file once per service in it would be the shape this whole type exists to avoid.
    pub(crate) fn locate_path(
        &mut self,
        path: &std::path::Path,
        range: TextRange,
    ) -> Option<Location> {
        let uri = Uri::from_file_path(path)?;
        if !self.read_paths.contains_key(path) {
            // The editor's copy first, for the reason every other reader here prefers it: a file
            // being edited is what the person asking is looking at.
            let text = match self.workspace.config_text(path) {
                Some(text) => Arc::clone(text),
                None => Arc::from(String::from_utf8_lossy(&std::fs::read(path).ok()?).into_owned()),
            };
            let lines = LineIndex::new(&text);
            self.read_paths.insert(path.to_path_buf(), (text, lines));
        }
        let (text, lines) = self.read_paths.get(path)?;
        Some(Location {
            uri,
            range: convert::range_of(text, lines, range),
        })
    }

    /// The URI of a file, for grouping edits by document.
    pub(crate) fn uri(&self, file: FileId) -> Option<Uri> {
        Uri::from_file_path(self.workspace.path_of(file))
    }

    fn text_of(&mut self, file: FileId) -> Option<(&str, &LineIndex)> {
        if !self.read.contains_key(&file) {
            self.read
                .insert(file, self.workspace.text_for_positions(file)?);
        }
        let (text, lines) = self.read.get(&file)?;
        Some((text, lines))
    }
}
