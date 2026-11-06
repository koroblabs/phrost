//! Whether a written type names one of the declaration's own `@template` parameters.
//!
//! A template nothing binds resolves to no class, so lowering it produces one the project does not
//! declare -- see [`Lowerer::doc_names_a_template`] and [`names_a_template`] for the two places that
//! matters: a plain tagged type, and a branch of a [`contract`](super::contract)'s conditional
//! return. Both read the same rule off the same text-match, which is why it has one home instead of
//! two.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{Element, TagValue},
};
use mago_span::HasSpan as _;

use crate::lowerer::Lowerer;

use super::{DocTag, slice, tagged_types_in};

impl Lowerer<'_, '_> {
    /// Whether any spelling of the type `tag` names above `declaration_start` is one of its own
    /// `@template` parameters.
    ///
    /// A template nothing binds resolves to no class, so lowering it produces one the project does
    /// not declare: `array_filter`'s `@return array<K, V>` would report every element of the result
    /// as a `V` object, and `in_array`'s `@param T $needle` would report every argument passed to it
    /// as taking a `T`. Reporting the plain `array` or `mixed` the signature already promised is
    /// less but is true. The same rule a conditional return branch follows -- see
    /// [`names_a_template`].
    ///
    /// Gated on the word appearing in the raw text. Locating the docblock is a lookup and parsing it
    /// is not, and outside a stub a `@template` on a declaration is rare.
    pub(crate) fn doc_names_a_template(
        &mut self,
        declaration_start: phrost_model::TextSize,
        tag: DocTag<'_>,
    ) -> bool {
        let Some(docblock) = self.raw_docblock_before(declaration_start) else {
            return false;
        };
        if !may_declare_a_template(docblock.text) {
            return false;
        }

        let text = docblock.text;
        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let templates = declared_templates(&document);
        tagged_types_in(&document, tag)
            .any(|ty| names_a_template(slice(text, ty.span()), &templates))
    }
}

/// Whether a docblock's raw text could hold a `@template` tag in any spelling.
///
/// The prefilter every template reader shares, and why it is one function: `@phpstan-template T`
/// does not contain `@template`, so a check for that literal missed every template a declaration
/// declared only with the prefix -- WordPress's `wp_unslash()` among them -- and its `T` was read as
/// a class nobody declares. A superset, as a prefilter may be: the parse that follows decides.
pub(crate) fn may_declare_a_template(text: &str) -> bool {
    text.contains("@template") || text.contains("-template")
}

/// The names a docblock's own `@template` tags declare, in the order written.
///
/// One place rather than three, because every caller of [`names_a_template`] needs exactly this list
/// and collecting it differently anywhere would make the same tag a template in one read and a class
/// in another.
pub(crate) fn declared_templates<'arena>(
    document: &mago_phpdoc_syntax::cst::Document<'arena>,
) -> Vec<std::borrow::Cow<'arena, str>> {
    document
        .elements
        .iter()
        .filter_map(|element| match element {
            Element::Tag(entry) => match &entry.value {
                TagValue::Template(value) => Some(String::from_utf8_lossy(value.name.value)),
                _ => None,
            },
            Element::Text(_) | Element::Code(_) => None,
        })
        .collect()
}

/// The keyword a `@template` names as its default -- `@template TDefault = never` -- when the
/// docblock declares `template` and gives it one.
///
/// Only a keyword. What it serves is a call that binds nothing, and the one thing such a call can be
/// handed is a value with no class: `never` for an operand that is not there, `null`, `mixed`. A
/// default naming a class is a different claim, and a *class's* template defaults are not read at
/// all -- `docs/08-types.md` says why.
pub(crate) fn default_keyword(
    document: &mago_phpdoc_syntax::cst::Document<'_>,
    text: &str,
    template: &str,
) -> Option<phrost_model::BuiltinTy> {
    document.elements.iter().find_map(|element| match element {
        Element::Tag(entry) => match &entry.value {
            TagValue::Template(value) if value.name.value == template.as_bytes() => {
                let default = value.default?;
                phrost_model::BuiltinTy::from_name(slice(text, default.r#type.span()).trim())
            }
            _ => None,
        },
        Element::Text(_) | Element::Code(_) => None,
    })
}

/// Whether a branch of a conditional return type names one of the declaration's own templates.
///
/// Read off the branch's own text rather than by walking the type, because only the occurrence
/// matters and the text is exact: `array<K, V>` names `V` wherever in the shape it sits, and every
/// way of writing a container would otherwise need its own arm.
///
/// Whole words only. `Value` is not `V`, and a template called `T` would otherwise match every
/// `Traversable` in sight.
pub(crate) fn names_a_template(written: &str, templates: &[std::borrow::Cow<'_, str>]) -> bool {
    templates.iter().any(|template| {
        template.len() <= written.len()
            && written.match_indices(template.as_ref()).any(|(at, _)| {
                let before = written[..at].chars().next_back();
                let after = written[at + template.len()..].chars().next();
                !before.is_some_and(is_name_char) && !after.is_some_and(is_name_char)
            })
    })
}

fn is_name_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '\\')
}
