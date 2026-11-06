//! A constructor parameter that could declare the property it fills.
//!
//! `__construct(private string $title)` is one declaration PHP reads as two, and it is how modern
//! PHP writes a constructor -- 1 508 of them in 431 files on one real checkout. Turning the older
//! three-part spelling into it is three edits at once: the property goes, the parameter gains the
//! visibility, and the assignment in the body goes with them.
//!
//! # Everything it refuses, and why each
//!
//! Getting one of these wrong changes what the class means, and nothing would fail until it ran. So
//! the shape has to be exactly the one promotion is equivalent to:
//!
//! - **A variadic or by-reference parameter.** PHP forbids promoting either.
//! - **A property whose type is spelled differently from the parameter's.** `private ?string` filled
//!   by a `string` parameter becomes a `private string`, which is a narrower property than the class
//!   declared -- and an untyped property given the parameter's type is the same loss from further
//!   away. Equal spellings, `None` included, and nothing else.
//! - **A property with a default.** The parameter's default would become the property's, and where
//!   only the property has one it is simply lost.
//! - **A static or hooked property.** Neither is a slot promotion can produce.
//! - **A property whose docblock says anything but `@var`.** The comment is deleted with the
//!   declaration, and a sentence somebody wrote is not this action's to throw away. A bare `@var`
//!   repeats the type and goes with it.
//! - **A body that does anything else with the slot.** The assignment removed has to be exactly
//!   `$this->x = $x;`, alone on its line: `$this->x = trim($x);` is not promotion, and a line
//!   holding two statements cannot be cut at a line boundary.

use phrost_body::{AssignmentTarget, PathRoot};
use phrost_hir::MemberKind;
use phrost_model::{Name, TextRange, TextSize};

use crate::view::IndexView;

/// A parameter that could take its property's declaration over, and the three edits that do it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Promotable {
    pub name: Name,
    /// The property declaration to remove: its docblock, its own line, and the blank line under it.
    pub property: TextRange,
    /// What goes in front of the parameter, the trailing space included: `private `,
    /// `protected readonly `.
    pub modifiers: String,
    /// Where that goes: the first byte of the parameter as written, which is its type when it has
    /// one and its `$` when it does not.
    pub at: TextSize,
    /// The `$this->x = $x;` line, newline included.
    pub assignment: TextRange,
}

impl IndexView<'_> {
    /// The promotion available at `offset`, when the cursor is on a constructor parameter that has
    /// one.
    pub fn promotable_at(&self, offset: TextSize) -> Option<Promotable> {
        let open = self.current()?;
        let hir = open.hir();
        let text = open.text();

        let (_, class) = hir
            .classes()
            .find(|(_, class)| class.full_range.contains_inclusive(offset))?;
        let members = hir.member_list(class.members);

        let constructor = members.iter().map(|&id| hir.member(id)).find(|member| {
            member.kind == MemberKind::Method
                && member.name.as_str().eq_ignore_ascii_case("__construct")
        })?;
        let param = hir
            .param_list(constructor.params)
            .iter()
            .map(|&id| hir.param(id))
            .find(|param| param.name_range.contains_inclusive(offset))?;
        if param.is_variadic || param.by_reference || param.promotion.is_some() {
            return None;
        }

        // The property this parameter fills, compared the way PHP compares one: exactly.
        let property = members.iter().map(|&id| hir.member(id)).find(|member| {
            member.kind == MemberKind::Property && member.name.as_str() == param.name.as_str()
        })?;
        if property.modifiers.is_static() || property.modifiers.hooks.are_written() {
            return None;
        }
        let written = text.get(
            usize::from(property.full_range.start())..usize::from(property.full_range.end()),
        )?;
        if written.contains('=') {
            return None;
        }

        let rendered = |written: Option<phrost_hir::TypeRefId>| {
            written.map(|id| phrost_hir::render::render_type_ref(hir, id))
        };
        if rendered(property.type_ref) != rendered(param.type_ref) {
            return None;
        }
        if property
            .docblock
            .map(|id| hir.docblock(id))
            .is_some_and(|docblock| says_more_than_a_var(&docblock.text))
        {
            return None;
        }

        let body = constructor.body?;
        let assignment = self.filling_line(hir, text, hir.body(body).range, param.name.as_str())?;

        let mut modifiers = property.visibility.as_str().to_owned();
        modifiers.push(' ');
        if property.modifiers.is_readonly() {
            modifiers.push_str("readonly ");
        }
        Some(Promotable {
            name: param.name.clone(),
            property: whole_lines(
                text,
                property_starts_at(hir, property),
                property.full_range.end(),
            ),
            modifiers,
            at: param.full_range.start(),
            assignment,
        })
    }

    /// The one line in `body` that assigns the parameter straight into its own slot.
    ///
    /// Exactly one, and alone on its line. A second assignment to the same slot means the body does
    /// something promotion does not, and a line holding another statement cannot be cut at a line
    /// boundary without taking that one with it.
    fn filling_line(
        &self,
        hir: &phrost_hir::FileHir,
        text: &str,
        body: TextRange,
        name: &str,
    ) -> Option<TextRange> {
        let mut found = None;
        for site in phrost_body::assignment_sites(hir) {
            if !body.contains_range(site.target_range) {
                continue;
            }
            let AssignmentTarget::Property {
                receiver,
                name: slot,
            } = &site.target
            else {
                continue;
            };
            if slot.as_str() != name {
                continue;
            }
            // A second one means the body is doing more than promotion would.
            if found.is_some() {
                return None;
            }
            if receiver.root != PathRoot::This || !receiver.steps.is_empty() {
                return None;
            }
            // The value has to be the parameter itself, not something computed from it.
            let value = site.value.as_ref()?;
            let PathRoot::Local(local) = &value.root else {
                return None;
            };
            if local.as_str() != name || !value.steps.is_empty() {
                return None;
            }
            found = Some(alone_on_its_line(
                text,
                site.target_range,
                site.value_range,
            )?);
        }
        found
    }
}

/// The range a whole statement occupies, when nothing else shares its lines.
fn alone_on_its_line(text: &str, target: TextRange, value: TextRange) -> Option<TextRange> {
    let from = text[..usize::from(target.start())]
        .rfind('\n')
        .map_or(0, |at| at + 1);
    if !text[from..usize::from(target.start())]
        .chars()
        .all(char::is_whitespace)
    {
        return None;
    }
    let after = usize::from(value.end());
    let end = text[after..]
        .find('\n')
        .map_or(text.len(), |at| after + at + 1);
    // Between the value and the line's end there may be only the `;` that terminates it.
    if text[after..end].trim_end().trim_end_matches(';').trim() != "" {
        return None;
    }
    Some(TextRange::new(
        TextSize::try_from(from).ok()?,
        TextSize::try_from(end).ok()?,
    ))
}

/// Where a property's declaration begins for the purpose of removing it: its docblock, when it has
/// one, and its own first byte otherwise.
fn property_starts_at(hir: &phrost_hir::FileHir, property: &phrost_hir::MemberData) -> TextSize {
    property
        .docblock
        .map(|id| hir.docblock(id).range.start())
        .unwrap_or(property.full_range.start())
}

/// The lines `from..to` sits on, plus one blank line under them.
///
/// Whole lines, because removing a declaration and leaving its indentation behind is a line of
/// spaces -- and the blank line under it, because a property block is written with one between each
/// and removing the declaration alone leaves two.
fn whole_lines(text: &str, from: TextSize, to: TextSize) -> TextRange {
    let start = text[..usize::from(from)].rfind('\n').map_or(0, |at| at + 1);
    let after = usize::from(to);
    let mut end = text[after..]
        .find('\n')
        .map_or(text.len(), |at| after + at + 1);
    if text[end..]
        .lines()
        .next()
        .is_some_and(|line| line.trim().is_empty())
    {
        end += text[end..].find('\n').map_or(0, |at| at + 1);
    }
    TextRange::new(
        TextSize::try_from(start).unwrap_or_default(),
        TextSize::try_from(end).unwrap_or_default(),
    )
}

/// Whether a docblock says anything a property's type does not already.
///
/// A bare `/** @var string */` repeats the declaration and goes with it. A sentence is somebody's
/// and is not this action's to delete.
fn says_more_than_a_var(text: &str) -> bool {
    text.lines()
        .map(|line| {
            // The closing delimiter first, or stripping the leading `*` off `*/` leaves a `/` that
            // reads as prose -- and every docblock has that line, so nothing would ever be offered.
            line.trim()
                .trim_end_matches("*/")
                .trim_start_matches("/**")
                .trim_start_matches('*')
                .trim()
        })
        .any(|content| !content.is_empty() && !content.starts_with("@var"))
}
