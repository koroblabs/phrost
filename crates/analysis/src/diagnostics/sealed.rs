//! A class that extends one the author closed.
//!
//! `Class Cents cannot extend final class Money` is a fatal at class load, so the file does not run
//! at all -- and what forbids it is written in another file, which is why an index answers it and a
//! linter cannot. The shape it is for is a parent made `final` while a subclass was already
//! written: one file changed, another stopped loading.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const SEALED_PARENT: &str = "sealed-parent";

pub(super) struct SealedParent;

impl Check for SealedParent {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.sealed_parents() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: SEALED_PARENT,
                message: format!("{} is final and cannot be extended", found.parent),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
