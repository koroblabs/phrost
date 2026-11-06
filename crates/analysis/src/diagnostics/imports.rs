//! A `use` line nothing in the file names.
//!
//! The diagnostic wrapper around [`IndexView::unused_imports`](phrost_view::IndexView), which does
//! the actual work of deciding which import nothing in the file binds through -- see that module's
//! docs for why it is not built from `name_references`, why the def-map decides, and the four
//! constructs it still cannot see an import used from. This file only turns its answer into a
//! [`Diagnostic`].
//!
//! Like `unused-variable` and unlike the rest of the checks here, nothing about the program is
//! wrong: an unused import runs exactly as written. It is something to delete, not something to
//! stare at, which is why it is a warning tagged `unnecessary` -- editors fade it instead of
//! underlining it.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

/// Named because a "remove this import" quick fix will want to name it.
pub(crate) const UNUSED_IMPORT: &str = "unused-import";

pub(super) struct UnusedImport;

impl Check for UnusedImport {
    /// An import is unused because nothing *anywhere* in the file binds through it, so text the
    /// parser skipped would read as nothing using it -- the same shape as `unused::UnusedLocal`.
    /// In practice the projection already declines to answer at all once the file has a parse
    /// error, but the check states the same requirement rather than relying on that silently.
    fn needs_a_whole_file(&self) -> bool {
        true
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for unused in view.unused_imports() {
            out.push(Diagnostic {
                range: unused.range,
                // A warning rather than an error, and faded rather than underlined: nothing about
                // the program is wrong.
                severity: Severity::Warning,
                code: UNUSED_IMPORT,
                message: format!("Unused import {}", unused.path),
                unnecessary: true,
                deprecated: false,
            });
        }
    }
}
