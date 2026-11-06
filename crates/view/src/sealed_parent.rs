//! A class that extends one the author closed.
//!
//! `final class Money {}` beside `class Cents extends Money {}` is a fatal error at *class load*,
//! before any of it runs -- `Class Cents cannot extend final class Money` -- and it is the same
//! kind of claim as [`unimplemented_methods`](IndexView::unimplemented_methods): what the class is
//! not allowed to do is written in another file, so nothing but an index can see it.
//!
//! The shape it is for is not a typo. It is a vendor class somebody tried to extend and a parent
//! somebody made `final` while a subclass was already written -- the second of which is a change in
//! one file that breaks another, which is exactly what a language server exists to notice before a
//! page is reloaded.
//!
//! `readonly` is not the same claim and is not here: PHP allows a `readonly` class to be extended
//! by another `readonly` class, and the rule about that belongs with the rest of the readonly ones.

use phrost_model::{Name, TextRange};
use phrost_sema::ClassKind;

use crate::view::IndexView;

/// One class whose parent refuses to be extended.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SealedParent {
    /// The subclass's own name, which is what a report underlines.
    pub range: TextRange,
    pub class: Name,
    /// The parent, as it resolved.
    pub parent: Name,
}

impl IndexView<'_> {
    /// Every class in the current file that extends a `final` one.
    pub fn sealed_parents(&self) -> Vec<SealedParent> {
        let Some(file) = self.current_file() else {
            return Vec::new();
        };
        let Some(skeleton) = self.skeleton.file(file) else {
            return Vec::new();
        };
        let mut out = Vec::new();

        for class in skeleton.classes() {
            // Only a class extends a class. An interface's `extends` names interfaces, which PHP
            // has no `final` for, and an enum and a trait extend nothing at all.
            if class.kind() != ClassKind::Class {
                continue;
            }
            // The declaration that wins is where the report belongs, and where its range comes
            // from -- the same rule `unimplemented_methods` follows, for the same reason.
            let Some(location) = self.class_location(class.fqn().as_str()) else {
                continue;
            };
            if location.file != file {
                continue;
            }
            for parent in class.extends() {
                // Through the header rather than off the parent's own row, because `final class`
                // and PHP 8.2's `final` on a class it inherits are one question and the header is
                // where both are already answered. A parent this index does not hold answers
                // nothing, which is the same silence every check here keeps over a gap.
                let Some(header) = self.class_header(parent.fqn.as_str()) else {
                    continue;
                };
                // An enum is final and cannot be extended either, but `class C extends SomeEnum` is
                // refused for being an enum rather than for being final, and `uninstantiable-class`
                // is not the report for it. One claim per check: this one is about the word.
                if header.kind != ClassKind::Class || !header.is_final {
                    continue;
                }
                out.push(SealedParent {
                    range: location.name_range,
                    class: class.fqn().clone(),
                    parent: header.fqn,
                });
            }
        }

        out
    }
}
