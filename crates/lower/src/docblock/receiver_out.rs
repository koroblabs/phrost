//! `@phpstan-self-out T` and its spellings: what the receiver holds once the call has returned.
//!
//! `@param-out` turned the other way round. That tag says what a by-reference parameter leaves in
//! the caller's variable; this one says what a method leaves in the variable it was *called on* --
//! Laravel's `Collection::transform()` maps the collection in place and says so with
//! `@phpstan-this-out static<TKey, TMapValue>`, where `TMapValue` is whatever its callback returns.
//!
//! The type is read slot by slot through the reading a `@return Bar<T>` already has
//! ([`Lowerer::applied_arguments`]), because it is the same claim -- a class written down, with
//! arguments only the call can fill -- made about a different value.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{PHPDocParser, cst::r#type::Type};
use mago_span::HasSpan as _;

use crate::lowerer::Lowerer;

use super::DocTag;

impl Lowerer<'_, '_> {
    /// The class a method's self-out names and what fills each of its slots.
    ///
    /// A class reference only, with or without arguments: `self<U>`, `static<TKey, TMapValue>`, a
    /// subclass written outright. An intersection or a union names no single class to put the
    /// arguments on, and nothing in a real tree writes one.
    ///
    /// `None` when a slot names one of the method's own templates and no parameter binds it --
    /// Symfony's `setParent()` writes `@psalm-this-out static<TNewParent>` with nothing tying
    /// `TNewParent` to the argument. A slot filled with a class nobody declares is a wrong answer
    /// rather than a partial one, and the receiver keeping what it held is the honest weaker one.
    ///
    /// Gated on the words in the raw text, for the reason every contract here is: locating the
    /// docblock is a lookup and parsing it is not, and the tag is written on a handful of methods
    /// in a whole vendor tree.
    pub(crate) fn receiver_out(
        &mut self,
        declaration_start: phrost_model::TextSize,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
    ) -> Option<phrost_hir::ContractData> {
        let docblock = self.raw_docblock_before(declaration_start)?;
        if !docblock.text.contains("self-out") && !docblock.text.contains("this-out") {
            return None;
        }
        let text = docblock.text.to_owned();
        let base = docblock.range.start();
        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let Type::Reference(reference) = super::tagged_type_in(&document, DocTag::SelfOut)? else {
            return None;
        };
        let arguments = self.applied_arguments(&document, &text, params, None, reference, base)?;
        let range = super::shift(reference.kind.span(), base);
        let class = self.doc_reference_base(&reference.kind, range, &text);
        Some(phrost_hir::ContractData::ReceiverOut {
            class,
            arguments: arguments.into_boxed_slice(),
        })
    }
}
