//! `@assert`, `@assert-if-true`, and `@assert-if-false`.
//!
//! What these narrow is either the receiver (`$this`) or a parameter, and the two are read
//! differently: a receiver claim answers a [`DocTag`](super::DocTag) lookup like any other tag,
//! because there is exactly one receiver and the caller already knows which one. A parameter claim
//! cannot -- it names the parameter itself, so matching it to an argument at a call site is this
//! module's own job rather than the generic lookup's, which is why it has no `DocTag` variant and
//! is read here instead.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{
        Element, TagValue,
        tag::{AssertPattern, AssertSubject, AssertTagValue},
        r#type::Type,
    },
};
use mago_span::HasSpan as _;

use crate::lowerer::Lowerer;

use super::{slice, variable_name};

impl Lowerer<'_, '_> {
    /// What an `@assert` tag about a *parameter* claims, in any of its four spellings.
    ///
    /// Its own lookup rather than a [`DocTag`](super::DocTag), because what it yields is a
    /// parameter name beside a type rather than a type alone -- the tag is about the caller's
    /// variable, and matching it to an argument is the reader's job.
    ///
    /// The first such tag wins. A declaration asserting about two parameters at once is rare enough
    /// that carrying a list would cost every method a vector to hold none.
    pub(crate) fn asserted_about_a_parameter(
        &mut self,
        declaration_start: phrost_model::TextSize,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
    ) -> Option<phrost_hir::ContractData> {
        let docblock = self.raw_docblock_before(declaration_start)?;
        let base = docblock.range.start();
        let text = docblock.text.to_owned();
        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let (parameter, ty, negated, when_true) =
            document.elements.iter().find_map(|element| match element {
                Element::Tag(entry) => {
                    let (value, when_true) = match &entry.value {
                        TagValue::Assert(value) => (value, None),
                        TagValue::AssertIfTrue(value) => (value, Some(true)),
                        TagValue::AssertIfFalse(value) => (value, Some(false)),
                        _ => return None,
                    };
                    let (name, ty) = asserted_about_parameter(value)?;
                    Some((name, ty, value.bang.is_some(), when_true))
                }
                Element::Text(_) | Element::Code(_) => None,
            })?;

        let asserted_name = match ty {
            Type::Reference(reference) => Some(slice(&text, reference.kind.span()).to_owned()),
            _ => None,
        };
        let from_class_string = asserted_name
            .and_then(|name| self.bound_by_a_class_string(&document, &text, params, name.as_str()));
        let parameter = self.interner.name(parameter);
        let ty = self.lower_doc_type(ty, base, &text, &std::rc::Rc::clone(&self.class_aliases))?;
        Some(phrost_hir::ContractData::Argument {
            parameter,
            ty,
            negated,
            when_true,
            from_class_string,
        })
    }
}

/// The type an assert tag names, when it is about the receiver.
///
/// Three refusals, and each is a claim this representation cannot hold rather than a parse failure.
/// `!T` says the value is *not* a `T`, which names no class. A truthy or non-empty pattern names no
/// class either. And a tag about a parameter is about the caller's variable, which needs the argument
/// matched to the parameter -- a different job from narrowing a receiver.
pub(crate) fn asserted_about_this<'arena>(
    value: &AssertTagValue<'arena>,
) -> Option<&'arena Type<'arena>> {
    if value.bang.is_some() {
        return None;
    }
    let AssertPattern::Type(ty) = value.pattern else {
        return None;
    };
    let AssertSubject::Parameter { variable } = &value.subject else {
        return None;
    };
    (variable_name(variable) == "this").then_some(ty)
}

/// The parameter an `@assert` tag is about, and the type it names.
///
/// Two refusals, and each is a claim this representation cannot hold rather than a parse failure. A
/// truthy or non-empty pattern names no type. And a tag about `$x->y()` is about an expression, not
/// about something the caller passed, so there is no argument to match it to.
///
/// `$this` is excluded because the receiver form is the more specific claim and is read first: a tag
/// about it says what the value *is*, with no argument to line up.
fn asserted_about_parameter<'arena>(
    value: &AssertTagValue<'arena>,
) -> Option<(&'arena str, &'arena Type<'arena>)> {
    let AssertPattern::Type(ty) = value.pattern else {
        return None;
    };
    let AssertSubject::Parameter { variable } = &value.subject else {
        return None;
    };
    let name = variable_name(variable);
    (name != "this").then_some((name, ty))
}
