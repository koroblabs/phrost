//! Every global constant the current file names, and where the file imports one.
//!
//! PHP's third symbol table, beside the classes and the functions, with the same two-step lookup a
//! function gets: a `use const` import supplies the prefix when one matches, and an unqualified
//! name inside a namespace falls back to global when `App\MAX` is declared nowhere.
//!
//! Here for find-references and rename, which had nothing to ask. A constant declaration was
//! reached as a class-shaped target, and nothing that answers about classes answers about one -- so
//! find-references at a `const MAX = 10;` listed nothing at all, not even the declaration, and the
//! rename offered at that cursor came back with an empty edit and no error. An editor opens its
//! dialogue, takes a new name, and does nothing.

use phrost_hir::ImportKind;
use phrost_model::TextRange;

use crate::view::IndexView;

impl IndexView<'_> {
    /// Every place the current file writes the name of one global constant, in source order.
    ///
    /// Ranges rather than a projection type: nothing above this needs the written path or whether
    /// the name resolved, because there is no undefined-constant check to feed -- and a list of
    /// facts nobody reads is a list that goes stale.
    pub fn constant_references(&self, fqn: &str) -> Vec<TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();

        for (_, reference) in hir.name_refs() {
            if reference.kind != ImportKind::Constant {
                continue;
            }
            let resolved = self.resolve_name(
                reference.range.start(),
                reference.path.as_str(),
                reference.anchor,
                ImportKind::Constant,
            );
            if resolved.is_some_and(|location| location.fqn.as_str() == fqn) {
                out.push(reference.range);
            }
        }
        out.extend(self.constant_imports_of(fqn));

        out.sort_by_key(|range| range.start());
        out.dedup();
        out
    }

    /// Where the current file writes `use const <fqn>;`.
    ///
    /// Case-sensitively, unlike a function's import: PHP compares a constant's name exactly, which
    /// is the one place the three tables disagree about what the same name means.
    pub fn constant_imports_of(&self, fqn: &str) -> Vec<TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        open.hir()
            .imports()
            .filter(|(_, import)| {
                import.kind == ImportKind::Constant && import.path.as_str() == fqn
            })
            .map(|(_, import)| import.path_range)
            .collect()
    }
}
