use std::sync::Arc;

use phrost_defmap::FileScope;
use phrost_hir::FileHir;
use phrost_lower::{KeyedAttributes, SyntaxError};
use phrost_model::{FileId, Interner, LineIndex};

/// One open editor buffer, analysed in full.
///
/// This is the second tier. The skeleton knows what every file in the project *declares*; this
/// knows everything about the one file the user is looking at, including every name written inside
/// it and where it sits.
///
/// Both halves are needed and neither is redundant. A position query — "what is under the cursor" —
/// can only ever be about a file that is open, because that is the file being looked at. A
/// cross-file answer — "where is that declared" — comes from the skeleton, so it does not matter
/// whether the target file is open.
///
/// That split is what makes goto give the same answer whether the target file happens to be open,
/// which is the behaviour the previous attempt could not hold onto.
#[derive(Debug)]
pub struct OpenFile {
    file: FileId,
    text: Arc<str>,
    line_index: LineIndex,
    hir: FileHir,
    scope: FileScope,
    errors: Vec<SyntaxError>,
}

impl OpenFile {
    /// Parses, lowers, and scopes one buffer.
    ///
    /// Takes the interner so names are shared with the rest of the project rather than duplicated
    /// per open document.
    pub fn analyze(
        interner: &mut Interner,
        file: FileId,
        text: impl Into<Arc<str>>,
        attributes: &KeyedAttributes,
    ) -> Self {
        let text: Arc<str> = text.into();
        // With bodies: this is the one file the user is looking at, and everything about a local
        // variable, a call site, or a member access site is inside one.
        let lowered = phrost_lower::lower_text_with_bodies(interner, file, &text, attributes);
        let scope = FileScope::build(interner, &lowered.hir);

        Self {
            file,
            line_index: LineIndex::new(&text),
            text,
            hir: lowered.hir,
            scope,
            errors: lowered.errors,
        }
    }

    pub fn file(&self) -> FileId {
        self.file
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    pub fn hir(&self) -> &FileHir {
        &self.hir
    }

    pub fn scope(&self) -> &FileScope {
        &self.scope
    }

    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }
}

/// The set of buffers currently open in the editor.
///
/// Deliberately a separate lifetime from the skeleton: closing a file drops its entry and nothing
/// else happens. In particular, closing must not trigger a disk read or a re-analysis — the
/// skeleton already knows what the file declares, so close is pure forgetting.
#[derive(Debug, Default)]
pub struct OpenSet {
    files: rustc_hash::FxHashMap<FileId, OpenFile>,
    /// Names for open buffers.
    ///
    /// Its own, rather than borrowed from the skeleton. A `Name` compares by content, so nothing
    /// depends on which interner produced it, and the duplication is bounded by how many files a
    /// person has open. The alternative -- handing out a mutable borrow of the skeleton's interner
    /// -- would make opening a buffer conflict with every read of the index.
    interner: Interner,
    /// Which attributes lowering keeps a positional string literal for.
    ///
    /// Configuration, set once from the same source the skeleton's copy comes from. The two must
    /// agree: a buffer lowered without the list declares no key, so opening a file would withdraw
    /// what the closed one claimed.
    keyed_attributes: KeyedAttributes,
    /// Templates held as text beside an open one: the ones rendering it with their variables, read
    /// for what they hand it. A template is not PHP and has no `FileId`, so it is kept by path.
    templates: rustc_hash::FxHashMap<Arc<std::path::Path>, Arc<str>>,
}

impl OpenSet {
    /// Holds a template's text beside an open one, replacing what was held for it.
    pub fn hold_template(&mut self, path: &std::path::Path, text: Arc<str>) {
        self.templates.insert(path.into(), text);
    }

    /// Lets go of a template held beside an open one.
    pub fn release_template(&mut self, path: &std::path::Path) {
        self.templates.remove(path);
    }

    /// The text of a template held beside an open one.
    pub fn template_text(&self, path: &std::path::Path) -> Option<&str> {
        self.templates.get(path).map(|text| &**text)
    }

    /// Every template held beside an open one.
    pub fn held_templates(&self) -> impl Iterator<Item = &std::path::Path> {
        self.templates.keys().map(|path| &**path)
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// Records which attributes a positional string literal is worth keeping for.
    pub fn set_keyed_attributes(&mut self, attributes: KeyedAttributes) {
        self.keyed_attributes = attributes;
    }

    pub fn open(&mut self, file: FileId, text: impl Into<Arc<str>>) {
        let analyzed = OpenFile::analyze(&mut self.interner, file, text, &self.keyed_attributes);
        self.files.insert(file, analyzed);
    }

    pub fn close(&mut self, file: FileId) -> Option<OpenFile> {
        self.files.remove(&file)
    }

    pub fn get(&self, file: FileId) -> Option<&OpenFile> {
        self.files.get(&file)
    }

    pub fn contains(&self, file: FileId) -> bool {
        self.files.contains_key(&file)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}
