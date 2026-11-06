//! `@template` parameters on a class-like declaration.
//!
//! PHP has no generics, so a docblock is the only place a container says what it holds. Drupal's
//! `FieldItemListInterface` is `@template T of FieldItemInterface` and nothing in the language
//! states it — which is why iterating one yielded nothing at all, on 149 positions of the corpus.
//!
//! Only the *bound*, not the argument. Substituting an argument through a chain of `@extends
//! ListInterface<T>` is a much larger job; a bound is what the declaration promises about every
//! argument it could be given, so using one offers fewer members than the real type has and never
//! more — a miss rather than a wrong answer, which is the only direction worth guessing in.

use lspkit_arena::IdSlice;
use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{Element, TagValue},
};
use mago_span::HasSpan as _;
use phrost_hir::{TemplateData, TemplateId};
use phrost_model::TextSize;

use crate::{
    docblock::{shift, type_alias::TypeAliasTable},
    lowerer::Lowerer,
};

impl Lowerer<'_, '_> {
    /// The classes a `@mixin` tag adds to this one's member lookup.
    ///
    /// PHP says nothing about them: the members answer at runtime through `__call`, and the tag is
    /// the only statement anywhere that they exist.
    ///
    /// `@require-extends` and `@require-implements` too, which say the same thing from the other
    /// side: every class composing this trait, or implementing this interface, *is* the class named.
    /// psl's `DateTimeConvenienceMethodsTrait` is `@require-implements DateTimeInterface` and calls
    /// `$this->getMonth()` a hundred times, all of which reached nothing. Read where a mixin is read:
    /// an interface's table takes the members in, and a trait's `$this` holds both -- see
    /// `IndexView::this_held`.
    pub(crate) fn docblock_mixins(
        &mut self,
        declaration_start: TextSize,
        aliases: &TypeAliasTable,
    ) -> IdSlice<phrost_hir::TypeRefId> {
        let Some(docblock) = self.raw_docblock_before(declaration_start) else {
            return IdSlice::EMPTY;
        };
        // Without the `@`, so a vendor-prefixed `@phpstan-mixin` is not filtered out -- it does
        // not contain `@mixin`, and a prefilter that misses a tag makes it invisible silently.
        if !docblock.text.contains("mixin") && !docblock.text.contains("require-") {
            return IdSlice::EMPTY;
        }

        let base = docblock.range.start();
        let text = docblock.text.to_owned();
        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let written = document
            .elements
            .iter()
            .filter_map(|element| match element {
                Element::Tag(tag) => match &tag.value {
                    TagValue::Mixin(value) => Some(value.r#type),
                    TagValue::RequireExtends(value) => Some(value.r#type),
                    TagValue::RequireImplements(value) => Some(value.r#type),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();

        let mut ids = Vec::new();
        for ty in written {
            if let Some(id) = self.lower_doc_type(ty, base, &text, aliases) {
                ids.push(id);
            }
        }
        self.builder.alloc_type_ref_list(&ids)
    }

    /// What the declaration supplies for a supertype's template parameters.
    ///
    /// `@extends Bar<Baz>` and its two siblings. Only the tags that carry arguments: one without
    /// them repeats the `extends` clause and says nothing this layer needs.
    pub(crate) fn docblock_supertype_arguments(
        &mut self,
        declaration_start: TextSize,
        aliases: &TypeAliasTable,
    ) -> IdSlice<phrost_hir::TypeRefId> {
        let Some(docblock) = self.raw_docblock_before(declaration_start) else {
            return IdSlice::EMPTY;
        };
        // The same prefilter shape the templates use, and it matters as much: a class writing one of
        // these is rare and every class has a docblock.
        //
        // Without the `@`, because the vendor-prefixed spellings are the ones some codebases use --
        // `@template-extends` does not contain `@extends`, and a prefilter that missed it made the
        // tag invisible with nothing to notice.
        // `use` carries its `@` where the other two do not, and that was an inconsistency rather
        // than a rule: `@template-use Holder<Widget>` -- the spelling Psalm's own docs use -- holds
        // neither `@use` nor `extends`, so the tag was invisible with nothing to notice. Both
        // spellings, and not a bare `use`, which every docblock mentioning the keyword would match.
        if !docblock.text.contains("extends")
            && !docblock.text.contains("implements")
            && !docblock.text.contains("@use")
            && !docblock.text.contains("template-use")
        {
            return IdSlice::EMPTY;
        }

        let base = docblock.range.start();
        let text = docblock.text.to_owned();
        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let written = document
            .elements
            .iter()
            .filter_map(|element| match element {
                Element::Tag(tag) => match &tag.value {
                    TagValue::Extends(value) => Some(value.r#type),
                    TagValue::Implements(value) => Some(value.r#type),
                    TagValue::Use(value) => Some(value.r#type),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();

        let mut ids = Vec::new();
        for ty in written {
            // Only a type with arguments. `@extends Bar` alone is the `extends` clause again.
            if let Some(id) = self.lower_doc_type(ty, base, &text, aliases)
                && matches!(
                    self.builder.type_ref(id),
                    phrost_hir::TypeRefData::Generic { .. }
                )
            {
                ids.push(id);
            }
        }
        self.builder.alloc_type_ref_list(&ids)
    }

    /// The supertype arguments with the parameterised mixins beside them.
    ///
    /// A `@mixin Builder<TRelatedModel>` supplies the mixin's templates the way an `@extends`
    /// supplies a parent's, and is read the same way -- see `IndexView::type_argument`, which walks
    /// a mixin as it walks a parent. The ids `docblock_mixins` already lowered, rather than the tag
    /// lowered again: lowering records every name it meets for the undefined-type check, and a
    /// second pass reported a missing mixin twice.
    pub(crate) fn with_mixin_arguments(
        &mut self,
        supertype_arguments: IdSlice<phrost_hir::TypeRefId>,
        mixins: IdSlice<phrost_hir::TypeRefId>,
    ) -> IdSlice<phrost_hir::TypeRefId> {
        let generic = self
            .builder
            .type_ref_list(mixins)
            .iter()
            .copied()
            .filter(|&id| {
                matches!(
                    self.builder.type_ref(id),
                    phrost_hir::TypeRefData::Generic { .. }
                )
            })
            .collect::<Vec<_>>();
        if generic.is_empty() {
            return supertype_arguments;
        }
        let mut all = self.builder.type_ref_list(supertype_arguments).to_vec();
        all.extend(generic);
        self.builder.alloc_type_ref_list(&all)
    }

    /// The `@template` parameters of the declaration starting at `declaration_start`.
    pub(crate) fn docblock_templates(
        &mut self,
        declaration_start: TextSize,
    ) -> IdSlice<TemplateId> {
        let Some(docblock) = self.raw_docblock_before(declaration_start) else {
            return IdSlice::EMPTY;
        };
        // Parsing is the expensive part and most docblocks hold prose, so the cheap check comes
        // first -- the same prefilter the docblock members use.
        // Without the `@`: `@phpstan-template` does not contain `@template`.
        if !docblock.text.contains("template") {
            return IdSlice::EMPTY;
        }

        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(docblock.text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let base = docblock.range.start();
        let mut ids = Vec::new();
        for element in document.elements.iter() {
            let Element::Tag(tag) = element else {
                continue;
            };
            // `@template-covariant` and the rest carry the same information for this purpose:
            // variance decides what may be substituted, not what the bound is.
            let TagValue::Template(value) = &tag.value else {
                continue;
            };

            let name = self
                .interner
                .name(&String::from_utf8_lossy(value.name.value));
            // `EMPTY`: same ordering reason as `docblock_mixins` above.
            let bound = value.bound.and_then(|bound| {
                self.lower_doc_type(
                    bound.r#type,
                    base,
                    docblock.text,
                    &std::rc::Rc::clone(&self.class_aliases),
                )
            });

            ids.push(self.builder.add_template(TemplateData {
                name,
                bound,
                name_range: shift(value.name.span(), base),
            }));
        }

        self.builder.alloc_template_list(&ids)
    }
}
