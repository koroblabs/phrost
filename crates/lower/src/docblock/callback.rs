//! `@param callable(A, B): C` -- what a documented callback will be *called with*.
//!
//! PHP has no syntax for it, so the docblock is the only place it is ever written, and the type
//! [`super::type_lowering`] builds from that tag keeps only the return: a `callable(A, B): C`
//! lowers to `Generic { Callable, [C] }` and `A` and `B` are read for their names and dropped.
//! That is right for the slot it lands in -- `ParamData::type_ref` answers *what may be passed
//! here*, and a caller passing `fn ($x) => ...` satisfies it whatever `$x` turns out to be.
//!
//! The parameters answer a different question -- *what will the thing I pass be handed* -- so they
//! get a slot of their own rather than a wider type. See
//! [`ParamData::callback_params_type_ref`](phrost_hir::ParamData::callback_params_type_ref) for
//! why the slot is what makes the shape unambiguous.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{PHPDocParser, cst::r#type::Type};
use mago_span::HasSpan as _;
use phrost_hir::{TypeRefData, TypeRefId};
use phrost_model::BuiltinTy;

use crate::lowerer::Lowerer;

use super::type_alias::TypeAliasTable;
use super::{DocTag, tagged_type_in};

impl Lowerer<'_, '_> {
    /// The types a documented callback's own parameters take, as a `Generic { Callable, [..] }`.
    ///
    /// `None` when the tag is absent, names no callable, or names one with no parameter list --
    /// all of which are the same answer to the caller: the declaration does not say what the
    /// callback receives.
    ///
    /// # Why this parses the docblock again
    ///
    /// The lowered `@param` type has already thrown the parameters away, so there is nothing left
    /// to read them off -- the tag's own CST node is the only thing that still holds them, and it
    /// lives in an arena that does not outlive the parse. Every sibling here re-parses for the
    /// same reason, and the prefilter is what keeps it off the path: a callable specification
    /// cannot be written without `callable(`, `Closure(` or one of their `pure-` spellings, so the
    /// substring test below rejects all but a handful of declarations in a project before any
    /// parsing happens.
    pub(crate) fn doc_callback_params(
        &mut self,
        declaration_start: phrost_model::TextSize,
        tag: DocTag<'_>,
        aliases: &TypeAliasTable,
    ) -> Option<TypeRefId> {
        let docblock = self.raw_docblock_before(declaration_start)?;
        // Both cases of the first letter, and both keywords, in two substring tests: `callable(`,
        // `Callable(`, `Closure(`, `closure(`, and the `pure-` forms of each all contain one of
        // these. A docblock with a type this can read cannot fail the test.
        if !(docblock.text.contains("allable(") || docblock.text.contains("losure(")) {
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

        let written = tagged_type_in(&document, tag)?;
        let templates = super::template::declared_templates(&document);
        let held = |name: &str| super::argument_binding::a_container_holds(&document, &text, name);
        self.callback_params_of(written, base, &text, aliases, &templates, &held)
    }

    /// [`Self::doc_callback_params`] for a tag whose CST node is already in hand.
    ///
    /// Its own function because two tags carry a callable and only one of them has to be looked
    /// up: a `@method` tag's parameters are parsed by whoever is reading the `@method`, so
    /// re-locating and re-parsing the docblock to reach a node it already holds would be work with
    /// a known answer. Both go through this, so a rule added here reaches both -- which is §8's
    /// lesson about two constructors of one rule, applied before there were two.
    pub(crate) fn callback_params_of(
        &mut self,
        written: &Type<'_>,
        base: phrost_model::TextSize,
        text: &str,
        aliases: &TypeAliasTable,
        templates: &[std::borrow::Cow<'_, str>],
        held: &dyn Fn(&str) -> bool,
    ) -> Option<TypeRefId> {
        let spec = callable_in(written)?.specification.as_ref()?;

        let mixed = self
            .builder
            .add_type_ref(TypeRefData::Builtin(BuiltinTy::Mixed));
        let mut params = Vec::new();
        for entry in spec.parameters.entries.iter() {
            // A trailing `...` is dropped rather than modelled: keeping it would need the slot to
            // carry which position repeats, and the list is a plain type list on purpose. The cost
            // is that `callable(A, B...)` says nothing about a third parameter, which loses
            // precision and cannot be wrong -- where carrying it as a fixed `B` would answer the
            // fourth and fifth too, and be wrong the moment somebody widens the tag.
            if entry.ellipsis.is_some() {
                break;
            }
            // `mixed` where the position said nothing, so the list stays dense and a reader can
            // index it. Two ways to say nothing and they are the same answer: no type written at
            // all (`callable($a, Foo $b)`), and a type this representation cannot hold -- a
            // conditional, a `key-of`, a class constant wildcard. `mixed` is the honest spelling
            // of both, since every reader already treats it as no claim.
            let lowered = entry
                .parameter_type
                .as_ref()
                // A position naming one of the declaration's own `@template` parameters answers
                // nothing here, and which template it is decides what the call site may answer
                // instead. One a container `@param` holds -- `usort`'s `T` beside `array<T>
                // $array` -- is an element, and stored as `mixed` so the view's container reading
                // speaks. One nothing holds -- `array_reduce`'s accumulator `I` -- is kept as its
                // name, which the view refuses as a class and reads as "no element goes here".
                .filter(|written| {
                    let named = super::slice(text, written.span());
                    !super::template::names_a_template(named, templates) || !held(named)
                })
                .and_then(|written| self.lower_doc_type(written, base, text, aliases))
                .unwrap_or(mixed);
            params.push(lowered);
        }

        // Nothing said, so nothing stored. `callable()` documents a callback taking no arguments,
        // and a list of nothing but `mixed` is a tag that named a shape none of whose positions
        // this can hold -- neither is worth four bytes on the row or an entry in the type table.
        if params.is_empty() || params.iter().all(|&id| id == mixed) {
            return None;
        }

        let base_ty = self
            .builder
            .add_type_ref(TypeRefData::Builtin(BuiltinTy::Callable));
        let args = self.builder.alloc_type_ref_list(&params);
        Some(self.builder.add_type_ref(TypeRefData::Generic {
            base: base_ty,
            args,
            non_empty: false,
        }))
    }
}

/// The callable a `@param` names, through the wrappers that do not change what it is.
///
/// `?callable(A): void` and `(callable(A): void)` are the same claim about the same callback --
/// grouping and nullability say something about whether one is passed, not about what it receives.
/// A union of two callables is refused: `callable(A): void|callable(B): void` says the callback is
/// called one of two ways, and picking a side would state a position the tag did not. A union with
/// *one* is not ambiguous for a closure written at the call, which can only be that operand:
/// Eloquent's `where` is `(\Closure(static): mixed)|string|array|Expression $column`.
fn callable_in<'d>(
    written: &'d Type<'d>,
) -> Option<&'d mago_phpdoc_syntax::cst::r#type::CallableType<'d>> {
    match written {
        Type::Callable(callable) => Some(callable),
        Type::Nullable(nullable) => callable_in(nullable.inner),
        Type::Parenthesized(inner) => callable_in(inner.inner),
        Type::Union(_) => {
            let mut found = Vec::new();
            callables_in_union(written, &mut found);
            match found.as_slice() {
                [only] => Some(*only),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Every callable among a union's operands, through the wrappers `callable_in` looks through.
fn callables_in_union<'d>(
    written: &'d Type<'d>,
    into: &mut Vec<&'d mago_phpdoc_syntax::cst::r#type::CallableType<'d>>,
) {
    match written {
        Type::Union(union) => {
            callables_in_union(union.left, into);
            callables_in_union(union.right, into);
        }
        Type::Parenthesized(inner) => callables_in_union(inner.inner, into),
        Type::Nullable(nullable) => callables_in_union(nullable.inner, into),
        Type::Callable(callable) => into.push(callable),
        _ => {}
    }
}

impl Lowerer<'_, '_> {
    /// What `@param-closure-this` says `$this` is inside a closure passed as `name`.
    ///
    /// Behind a substring test for the reason [`Self::doc_callback_params`] gives: the tag is rare,
    /// and a docblock that does not spell it cannot hold it.
    pub(crate) fn doc_closure_this(
        &mut self,
        declaration_start: phrost_model::TextSize,
        name: &str,
        aliases: &TypeAliasTable,
    ) -> Option<TypeRefId> {
        let docblock = self.raw_docblock_before(declaration_start)?;
        if !docblock.text.contains("closure-this") {
            return None;
        }
        self.doc_type_with_aliases(declaration_start, DocTag::ParamClosureThis(name), aliases)
    }
}
