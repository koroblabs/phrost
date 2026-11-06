//! Whether a file declares the namespace the autoloader will look for it under.
//!
//! The one kind of mistake in a PHP project that nothing running the code will tell you about until
//! the moment it matters: a class whose namespace does not match its directory is simply never
//! found. `composer dump-autoload -o` hides it by writing a classmap, so it works on the machine
//! that dumped and fails on the one that did not -- and no compiler, no type checker and no test
//! that does not touch the class has anything to say.
//!
//! Two halves of the same rule, and both come from the autoload map rather than from a convention
//! this layer invented: the namespace a directory stands for, and the file name a class has to
//! match. Measured on one real checkout -- 1 651 files under a source root, two of them wrong, and
//! both were classes moved to another directory with their namespace left behind. Neither is
//! reachable today and nothing else in the project says so.
//!
//! # What it says nothing about
//!
//! - **A file under no source root.** A script, a template, something reached by a classmap entry
//!   or a `require`. The autoloader never looks for it by name, so there is nothing it must be
//!   called.
//! - **A file declaring more than one class-like.** PSR-4 asks for one per file and the rest do not
//!   autoload, but which of them the file is *named* after is the author's business -- and a
//!   fixture declaring five is not a mistake anybody wants reported five times.
//! - **A file declaring none.** There is nothing for the autoloader to find.

use phrost_model::{Name, TextRange};

use crate::view::IndexView;

/// A file the autoloader would not find, and what it would have to say instead.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Psr4Mismatch {
    /// The namespace does not match the directory.
    Namespace {
        /// The declaration's own range, or the position a missing one would go at.
        range: TextRange,
        declared: Option<Name>,
        required: String,
    },
    /// The class does not match the file name.
    ClassName {
        range: TextRange,
        declared: String,
        required: String,
    },
}

impl IndexView<'_> {
    /// What the current file gets wrong about where it lives, if anything.
    pub fn psr4_mismatch(&self) -> Option<Psr4Mismatch> {
        let open = self.current()?;
        let hir = open.hir();
        let file = self.current_file()?;
        let path = std::path::Path::new(self.file_path(file)?);
        // Every namespace a rule would accept, not one: PSR-4 roots overlap and a file is reachable
        // through any of them. The first is the most specific, and is what a message suggests.
        let permitted = self.skeleton.permitted_namespaces(path);
        let required = permitted.first()?.clone();

        // One class-like, which is what PSR-4 is about. A file declaring several is one the
        // autoloader finds by whichever name it was asked for, and only one of those can match.
        let mut classes = hir.classes().filter(|(_, class)| !class.is_anonymous);
        let (id, class) = classes.next()?;
        if classes.next().is_some() {
            return None;
        }

        let written = hir
            .namespace_at(class.full_range.start())
            .map(|id| hir.namespace(id));
        let name = written.and_then(|data| data.name.clone());
        let declared = name.as_ref().map_or("", |name| name.as_str());
        if !permitted.iter().any(|one| one == declared) {
            return Some(Psr4Mismatch::Namespace {
                // The name alone, which is what an edit replaces. A file with no declaration at all
                // is reported on its own class instead: there is no namespace to underline, and the
                // class is the thing that will not be found.
                range: written.map_or(class.name_range, |data| data.name_range),
                declared: name,
                required,
            });
        }

        let stem = path.file_stem()?.to_str()?;
        let short = self.declared_short_name(id)?;
        if short != stem {
            return Some(Psr4Mismatch::ClassName {
                range: class.name_range,
                declared: short,
                required: stem.to_owned(),
            });
        }
        None
    }

    /// The class's own name as the declaration writes it.
    ///
    /// From the declared symbol rather than the written node, for the reason `class_body_at` gives:
    /// the written one is what the source says and the symbol is what the catalog is keyed by, and
    /// only the second is guaranteed to be the whole name.
    fn declared_short_name(&self, id: phrost_hir::ClassId) -> Option<String> {
        let open = self.current()?;
        let symbol = open
            .scope()
            .symbol_of(phrost_defmap::DeclarationSource::Class(id))?;
        Some(phrost_model::short_name(symbol.fqn.as_str()).to_owned())
    }
}
