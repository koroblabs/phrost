//! `@return ID_TABLE[T]` beside `@param T $type`: the entry of a constant's table at the key a call
//! passes.
//!
//! PHPStan's offset access on a constant, and the one way a docblock can say that what comes back
//! depends on *which* key was asked for -- `lookUp('immutable')` is an `int` and `lookUp('mutable')`
//! a `string` when the table holds one of each. The declaration can only name the table and the
//! parameter; the table's value is in whichever file declares it and the key is written at the call,
//! so both halves are read where the whole index is known.
//!
//! A global constant (`ID_TABLE`) and a class's (`self::TABLE`, `Table::TABLE`) are one rule: the
//! table is a constant either way and only finding it differs.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::r#type::{MemberReferenceSelector, ReferenceKind, Type},
};
use mago_span::HasSpan as _;

use crate::lowerer::Lowerer;

use super::{DocTag, slice};

impl Lowerer<'_, '_> {
    /// The constant a `@return CONSTANT[T]` indexes, and the parameter its key template is declared
    /// as.
    ///
    /// Only a template a parameter is declared as outright: `T` is whatever the call passed, which is
    /// the key. A literal index -- `ID_TABLE['immutable']` -- is no claim about the call, and an index
    /// nothing binds is a key nothing here can name.
    ///
    /// Gated on a bracket and a template in the raw text, for the reason every contract here is: a
    /// docblock writing both is rare, and parsing is what costs.
    pub(crate) fn constant_entry(
        &mut self,
        declaration_start: phrost_model::TextSize,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
    ) -> Option<phrost_hir::ContractData> {
        let docblock = self.raw_docblock_before(declaration_start)?;
        if !docblock.text.contains(']') || !super::template::may_declare_a_template(docblock.text) {
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

        let Type::IndexAccess(access) = super::tagged_type_in(&document, DocTag::Return)? else {
            return None;
        };
        let Type::Reference(index) = access.index else {
            return None;
        };
        if index.parameters.is_some() {
            return None;
        }
        let key = slice(&text, index.kind.span());
        let templates = super::template::declared_templates(&document);
        if !templates.iter().any(|template| template == key) {
            return None;
        }
        let parameter = self.bound_by_a_parameter(&document, &text, params, key)?;

        let table = match access.target {
            // `ID_TABLE` -- a bare name in type position, which is a class to every other reader and
            // a constant here, where the brackets after it say so.
            Type::Reference(reference) if reference.parameters.is_none() => {
                let ReferenceKind::Identifier(identifier) = &reference.kind else {
                    return None;
                };
                let (anchor, path) = phrost_hir::NameAnchor::of(slice(&text, identifier.span()));
                phrost_hir::ConstantTableData::Global {
                    path: self.interner.name(path),
                    anchor,
                }
            }
            // `self::TABLE`, `Table::TABLE`. A wildcard names several constants and no one table.
            Type::MemberReference(member) => {
                let MemberReferenceSelector::Identifier(name) = &member.member else {
                    return None;
                };
                let range = super::shift(member.kind.span(), base);
                phrost_hir::ConstantTableData::Member {
                    class: self.doc_reference_base(&member.kind, range, &text),
                    name: self.interner.name(slice(&text, name.span())),
                }
            }
            _ => return None,
        };
        Some(phrost_hir::ContractData::ConstantEntry { table, parameter })
    }
}
