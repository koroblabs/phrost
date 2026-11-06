//! The files that render a template, lowered beside it.
//!
//! A template's variables are declared by the call that renders it --
//! `view('posts.show', ['post' => $post])` -- and what each one *is* is an expression in that call's
//! body. A closed file keeps no body, so whatever analyses a template has the files that render it
//! lowered next to it: the open set for a buffer the editor holds, the scratch set for one read from
//! disk. Both from [`Workspace::renderers`], so an open template and a closed one see the same
//! variables -- which is `open == closed` for what a template reaches.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use phrost_index::OpenSet;
use phrost_model::FileId;
use phrost_vfs::SourceText;

use super::Workspace;

impl Workspace {
    /// The files whose calls render the template at `path`, in file order, each once.
    pub(super) fn renderers(&self, path: &Path) -> Vec<FileId> {
        let Some(name) = self.skeleton.template_name(path) else {
            return Vec::new();
        };
        let mut files: Vec<FileId> = self
            .skeleton
            .renderers_of(&name)
            .into_iter()
            // A call handing nothing over has no value to read -- what it says is which class
            // renders, which the index already holds -- unless it hands over everything where it
            // is, which is read there.
            .filter(|(_, site)| !site.keys.is_empty() || site.inherits)
            .map(|(file, _)| file)
            .collect();
        files.dedup();
        files
    }

    /// Lowers the files that render the template `file` into `set`, beside it.
    pub(super) fn open_renderers(&self, file: FileId, set: &mut OpenSet) {
        for renderer in self.renderers_through(file) {
            if set.contains(renderer) {
                continue;
            }
            if let Some(text) = self.renderer_text(renderer) {
                set.open(renderer, text);
            }
        }
    }

    /// Every file a template's variables can be read through: what renders it, and -- where that
    /// is a template including this one -- what renders *that*, a few levels up. A partial's
    /// `$page` is whatever the page that includes it was handed.
    fn renderers_through(&self, file: FileId) -> Vec<FileId> {
        // As many as a template's variables are followed up through includes -- see the view's
        // `INCLUDED_LEVELS` -- since a level not lowered is one nothing can read.
        const LEVELS: usize = 4;
        let mut found: Vec<FileId> = Vec::new();
        let mut asking = vec![file];
        for _ in 0..LEVELS {
            let mut next = Vec::new();
            for template in asking {
                for renderer in self.renderers(self.vfs.path(template)) {
                    if renderer == file || found.contains(&renderer) {
                        continue;
                    }
                    found.push(renderer);
                    if self.is_template(self.vfs.path(renderer)) {
                        next.push(renderer);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            asking = next;
        }
        found
    }

    /// Keeps every file that renders an open template lowered in the open set -- and lets go of one
    /// no open template needs any more, unless the editor holds it.
    pub(super) fn settle_renderers(&mut self) {
        let mut needed: BTreeSet<FileId> = self
            .vfs
            .files()
            .filter(|&file| self.vfs.open_document(file).is_some())
            .filter(|&file| self.is_template(self.vfs.path(file)))
            .flat_map(|file| self.renderers_through(file))
            .collect();
        // And for a template held as text -- a Twig buffer, which has no `FileId` of its own -- the
        // templates including it, held as text beside it, and what renders each: its variables
        // are read out of the arrays rendering it and out of what the templates including it hand
        // over, a few levels up.
        let templates = self.templates_including_held();
        for path in self.configs.keys().chain(templates.iter()) {
            needed.extend(
                self.skeleton
                    .render_arrays_of(path)
                    .into_iter()
                    .map(|(file, ..)| file),
            );
        }
        let held: Vec<PathBuf> = self.open.held_templates().map(Path::to_path_buf).collect();
        for path in held {
            if !templates.contains(&path) {
                self.open.release_template(&path);
            }
        }
        for path in &templates {
            // The editor's buffer when it holds the template, which is what the person is looking
            // at; the disk's otherwise, read once -- a change to it on disk is the watcher's to
            // pass on, and this runs on every keystroke.
            let text: Option<Arc<str>> = match self.configs.get(path) {
                Some(buffer) => Some(Arc::clone(&buffer.text)),
                None if self.open.template_text(path).is_some() => continue,
                None => SourceText::read(path)
                    .ok()
                    .map(|source| source.shared_text()),
            };
            if let Some(text) = text {
                self.open.hold_template(path, text);
            }
        }
        let held_by_editor =
            |workspace: &Self, file: FileId| workspace.vfs.open_document(file).is_some();
        let released: Vec<FileId> = self
            .beside
            .iter()
            .copied()
            .filter(|file| !needed.contains(file) || held_by_editor(self, *file))
            .collect();
        for file in released {
            self.beside.remove(&file);
            if !held_by_editor(self, file) {
                self.open.close(file);
            }
        }
        for file in needed {
            if self.open.contains(file) {
                continue;
            }
            if let Some(text) = self.renderer_text(file) {
                self.open.open(file, text);
                self.beside.insert(file);
            }
        }
    }

    /// Every template including one held as text, and every one including those, a few levels up.
    fn templates_including_held(&self) -> BTreeSet<PathBuf> {
        // As many as the typer follows -- see the view's `LEVELS`.
        const LEVELS: usize = 4;
        let mut found: BTreeSet<PathBuf> = BTreeSet::new();
        let mut asking: Vec<PathBuf> = self.configs.keys().cloned().collect();
        for _ in 0..LEVELS {
            let mut next = Vec::new();
            for template in &asking {
                for including in self.skeleton.twig_includers_of(template) {
                    if found.insert(including.to_path_buf()) {
                        next.push(including.to_path_buf());
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            asking = next;
        }
        found
    }

    /// A template whose file changed on disk is read again for what it includes, and held again
    /// if it is held beside an open one -- which is all a change to a template not open can mean.
    pub(super) fn refresh_template(&mut self, path: &Path) {
        let Ok(source) = SourceText::read(path) else {
            self.skeleton.set_twig_includes(path, Vec::new());
            self.open.release_template(path);
            return;
        };
        self.skeleton.set_twig_includes(
            path,
            phrost_view::twig::included_template_names(source.text()),
        );
        if self.open.template_text(path).is_some() {
            self.open.hold_template(path, source.shared_text());
        }
        self.settle_renderers();
    }

    /// A renderer lowered beside a template whose file changed on disk is lowered again.
    pub(super) fn refresh_renderer(&mut self, file: FileId) {
        if self.beside.contains(&file)
            && let Some(text) = self.renderer_text(file)
        {
            self.open.open(file, text);
        }
    }

    /// The editor's buffer when it has one, or the file on disk -- a template read as PHP, as
    /// every reader of one does.
    fn renderer_text(&self, file: FileId) -> Option<Arc<str>> {
        if let Some(document) = self.vfs.open_document(file) {
            return Some(document.source().shared_text());
        }
        let path = self.vfs.path(file);
        let source = SourceText::read(path).ok()?;
        Some(match self.skeleton.as_php(path, source.text()) {
            Some(php) => Arc::from(php),
            None => source.shared_text(),
        })
    }
}
