//! What a `@see` points at, as a name the index can resolve.
//!
//! `@param`, `@return` and `@throws` name a class and are navigable because lowering records a name
//! reference for the type it read. `@see` names one too and was not, so ctrl-clicking
//! `@see \Drupal\Core\Entity\EntityInterface::id()` did nothing -- in a framework whose own source
//! writes the tag 12 282 times, and whose docblocks are how one class points at the four others it
//! only makes sense beside.
//!
//! # Why mago gives no help here, and how much that costs
//!
//! `@see` is not in mago's tag vocabulary, so it arrives as `TagValue::Generic` -- the tag name and
//! a span of text. That is still the parser's answer rather than a hand-rolled one: the star
//! stripping, the tag boundaries and the spans are all mago's, and the only thing read by hand is
//! which prefix of the value is the reference. That prefix is exactly the first token, because the
//! rest of a `@see` line is a description.
//!
//! # What is deliberately not judged
//!
//! Whether the token names anything. `@see Monica` and `@see i18n` are prose written in the
//! position a reference goes, and there is no rule that separates them from `@see LoggerCore`
//! without resolving -- which lowering cannot do. Emitting the reference anyway is safe because it
//! is marked `cross_reference`, and the one check that judges names in docblocks,
//! `doc-undefined-type`, leaves those out; `undefined-function` and the add-import action leave every
//! docblock name out. So the failure mode is goto answering nothing, which is what it answered
//! before.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{Element, Tag, TagValue, text::TextSegment},
};
use mago_span::HasSpan as _;
use phrost_hir::{ImportKind, NameAnchor, NameRefData};
use phrost_model::TextRange;
use phrost_syntax::ReferenceKind;

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_> {
    /// A name reference for every `@see` in the file that points at a class or a function.
    ///
    /// Every docblock rather than the one above each declaration: the tag is written above classes,
    /// methods, properties, functions and constants alike, and a pass installed per declaration kind
    /// is a pass missing wherever a kind was forgotten. The list is the parser's own and is already
    /// in position order.
    pub(crate) fn doc_sees_in_file(&mut self) {
        for docblock in self.raw_docblocks() {
            // The prefilter every reader here uses. Most docblocks carry no `@see`, and for those
            // the cost is one scan of text already in hand.
            if !docblock.text.contains("@see") {
                continue;
            }

            let base = docblock.range.start();
            let arena = LocalArena::new();
            let content = arena.alloc_slice_copy(docblock.text.as_bytes());
            // A synthetic file id, as elsewhere here: the parser only stamps spans with it, and
            // every span is rebased onto the real file.
            let document = PHPDocParser::parse(
                &arena,
                mago_database::file::FileId::new(b"docblock"),
                content,
            );

            let mut seen = Vec::new();
            for element in document.elements.iter() {
                match element {
                    Element::Tag(entry) => {
                        seen.extend(see_value(entry).map(|span| (span, 0)));
                        // A tag's own description holds inline tags too, and mago hands the
                        // description back inside whichever of its sixty tag values this is --
                        // twenty-five of which carry one, with no accessor in common. Reading the
                        // value's text back through the parser reaches every one of them, where a
                        // match over the variants that happen to have the field would silently miss
                        // the next variant added. Measured on one checkout: 545 of 12 400 `@see`
                        // are written this way, which is what makes a rule that cannot miss worth
                        // more than a faster one that can.
                        let at = entry.value.span().start.offset;
                        let inner = super::slice(docblock.text, entry.value.span());
                        if inner.contains("{@see") {
                            seen.extend(inline_sees(inner).into_iter().map(|span| (span, at)));
                        }
                    }
                    // `{@see Foo}` written mid-sentence in the docblock's own prose.
                    Element::Text(text) => seen.extend(
                        text.segments
                            .iter()
                            .filter_map(|segment| match segment {
                                TextSegment::InlineTag(inline) => see_value(inline.tag),
                                _ => None,
                            })
                            .map(|span| (span, 0)),
                    ),
                    Element::Code(_) => {}
                }
            }

            for (span, within) in seen {
                let from = (span.start.offset + within) as usize;
                let to = (span.end.offset + within) as usize;
                let Some(written) = docblock.text.get(from..to) else {
                    continue;
                };
                let Some((within, kind)) = phrost_syntax::see_reference(written) else {
                    continue;
                };
                let named = &written[usize::from(within.start())..usize::from(within.end())];
                let start = base
                    + phrost_model::TextSize::try_from(from).unwrap_or_default()
                    + within.start();
                let range = TextRange::new(start, start + within.len());
                let (anchor, path) = NameAnchor::of(named);
                let path = self.interner.name(path);
                self.add_name_ref(NameRefData {
                    path,
                    anchor,
                    kind: imported(kind),
                    range,
                    in_docblock: true,
                    cross_reference: true,
                });
            }
        }
    }
}

/// The value spans of every inline `{@see}` in a fragment, relative to the fragment.
///
/// The fragment is read back through the same parser rather than scanned, so what counts as an
/// inline tag stays mago's answer and there is one definition of it rather than two.
fn inline_sees(fragment: &str) -> Vec<mago_span::Span> {
    let arena = LocalArena::new();
    let content = arena.alloc_slice_copy(fragment.as_bytes());
    let document = PHPDocParser::parse(
        &arena,
        mago_database::file::FileId::new(b"docblock"),
        content,
    );

    document
        .elements
        .iter()
        .filter_map(|element| match element {
            Element::Text(text) => Some(text.segments),
            _ => None,
        })
        .flatten()
        .filter_map(|segment| match segment {
            TextSegment::InlineTag(inline) => see_value(inline.tag),
            _ => None,
        })
        .collect()
}

/// The span of a `@see` tag's value, block or inline.
///
/// `TagValue::Generic` is what mago produces for a tag it does not model, which `@see` is. Anything
/// else means mago learned the tag, and this should then be reading whatever it learned.
fn see_value(tag: &Tag<'_>) -> Option<mago_span::Span> {
    if !tag.name.value.eq_ignore_ascii_case(b"see") {
        return None;
    }
    match &tag.value {
        TagValue::Generic(value) => Some(value.value.span),
        _ => None,
    }
}

/// Which table the reference goes in, in this crate's vocabulary.
///
/// The spelling rule that decides it lives in `phrost-syntax`, because the token layer reads the
/// same values to colour them and two readers of one rule is how one of them comes to be missing
/// what the other has. Only the name of the answer differs between the two callers.
fn imported(kind: ReferenceKind) -> ImportKind {
    match kind {
        ReferenceKind::ClassLike => ImportKind::ClassLike,
        ReferenceKind::Function => ImportKind::Function,
        ReferenceKind::Constant => ImportKind::Constant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spelling rule and its tests live in `phrost-syntax`; what this crate decides is which
    /// of its own tables each answer names, and a kind added there must be given one here.
    #[test]
    fn every_kind_the_scanner_can_answer_names_a_table() {
        assert_eq!(imported(ReferenceKind::ClassLike), ImportKind::ClassLike);
        assert_eq!(imported(ReferenceKind::Function), ImportKind::Function);
        assert_eq!(imported(ReferenceKind::Constant), ImportKind::Constant);
    }
}
