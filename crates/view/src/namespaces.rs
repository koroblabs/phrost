//! What namespace a file's own code declares, and where it says so.
//!
//! Asked by one thing: a file being renamed. PSR-4 ties a class's namespace to the directory
//! holding it, so moving the file means the `namespace` line has to move with it -- and the
//! declaration is the only place that says which namespace the file is in, which is why the range
//! has to come from the tree rather than from a search for the word.

use phrost_model::TextRange;

use crate::IndexView;

/// A file's `namespace` declaration.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NamespaceDeclaration {
    /// What is written after the keyword, with no leading or trailing separator.
    pub name: String,
    /// The name alone, so replacing it leaves the keyword and the terminator where they are.
    ///
    /// For a file with no namespace at all this is the whole point of the type being an option
    /// rather than an empty string: there is nowhere to put one, and a caller that needs to
    /// introduce a declaration is doing something else.
    pub name_range: TextRange,
}

impl IndexView<'_> {
    /// Which line an offset in the current file is on, zero-based.
    ///
    /// For a tool that prints a position rather than sending one: the protocol layer has its own
    /// conversion and its own reasons, and a report on a terminal wants a line and nothing else.
    pub fn line_of(&self, offset: phrost_model::TextSize) -> u32 {
        self.current()
            .map_or(0, |open| open.line_index().line_col(offset).line)
    }

    /// The namespace the current file declares, when it declares exactly one.
    ///
    /// `None` for a file with none -- a script, a procedural include -- and for one with several
    /// blocks, which is legal PHP and is not a file PSR-4 can name: with two namespaces there is
    /// no single answer to "which namespace is this file", and a rename derived from the path
    /// would have to pick one.
    pub fn declared_namespace(&self) -> Option<NamespaceDeclaration> {
        let open = self.current()?;
        let named: Vec<_> = open
            .hir()
            .namespaces()
            .filter_map(|(_, namespace)| {
                Some(NamespaceDeclaration {
                    name: namespace.name.as_ref()?.as_str().to_owned(),
                    name_range: namespace.name_range,
                })
            })
            .collect();
        match named.len() {
            1 => named.into_iter().next(),
            _ => None,
        }
    }
}
