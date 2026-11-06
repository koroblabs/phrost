//! What a string is known to hold -- and so whether a call's argument passes a
//! `non-empty-string` or `non-falsy-string` test.
//!
//! WordPress keys `wpdb::get_row()` and `get_results()` on `$query is non-falsy-string`. Both
//! refinements are types of their own, so a value whose type says it holds something answers the
//! test the way it answers a parameter declared with one: through its type, which for a string the
//! call built in place -- quoted, interpolated, or joined from a quoted head and a variable tail -- is
//! what its syntax says. See `phrost_lower`'s `string_holding` and `PathRoot::Concatenation`.
//!
//! The one thing a type cannot say is that a string is *empty*: nothing keeps `''` as a type of its
//! own. A literal's text can, so a quoted string is read here, and is the only argument that fails a
//! test rather than leaving it unsettled. What an unsettled test means is the contract reader's to
//! decide.

use phrost_hir::{
    ExprId,
    body::{ExprData, LiteralKind},
};
use phrost_model::{BuiltinTy, TextRange, TextSize};

use super::CallFacts;
use crate::view::IndexView;

impl IndexView<'_> {
    /// Whether the expression a call passed is a string passing the test. `None` where neither its
    /// text nor its type says.
    pub(super) fn string_test_holds(
        &self,
        at: &CallFacts<'_>,
        written: ExprId,
        falsy_too: bool,
    ) -> Option<bool> {
        let wanted = match falsy_too {
            true => BuiltinTy::NonFalsyString,
            false => BuiltinTy::NonEmptyString,
        };
        match at.hir.expr(written) {
            ExprData::Literal {
                kind: LiteralKind::String,
                range,
            } => {
                let (holds, exact) = self.quoted_string_holds(*range)?;
                match holds.string_rank() >= wanted.string_rank() {
                    true => Some(true),
                    false => exact.then_some(false),
                }
            }
            // Null is no string, and neither is a number.
            ExprData::Literal {
                kind: LiteralKind::Null | LiteralKind::Int | LiteralKind::Float,
                ..
            } => Some(false),
            _ => {
                let path = phrost_body::value_path_of(at.hir, written)?;
                let held =
                    self.resolve_path(at.offset, at.hir, &path, at.budget.checked_sub(1)?)?;
                let holds = self.string_held(at.hir, &held)?;
                (holds.string_rank() >= wanted.string_rank()).then_some(true)
            }
        }
    }

    /// What a string literal holds, as a type: what its quoted text says, or what the magic constant
    /// it is always is. `__DIR__` and `__FILE__` are paths; `__CLASS__` inside a class is its name
    /// and outside one is empty, and the rest are empty somewhere this does not look for.
    pub(crate) fn literal_string_holds(&self, offset: TextSize, range: TextRange) -> BuiltinTy {
        if let Some((holds, _)) = self.quoted_string_holds(range) {
            return holds;
        }
        let Some(text) = self.text_in(range) else {
            return BuiltinTy::String;
        };
        if text.eq_ignore_ascii_case("__DIR__")
            || text.eq_ignore_ascii_case("__FILE__")
            || (text.eq_ignore_ascii_case("__CLASS__") && self.enclosing_class(offset).is_some())
        {
            return BuiltinTy::NonFalsyString;
        }
        BuiltinTy::String
    }

    /// What a string loosely equal to one of these literals holds: the least any of them does, and
    /// nothing past non-empty if one spells a number -- `'0' == '0.0'` holds, and `'0'` is falsy.
    /// No string but `''` is loosely equal to `''`, so a haystack without one rules it out.
    pub(crate) fn loosely_found_among(
        &self,
        hir: &phrost_hir::FileHir,
        written: &[ExprId],
    ) -> BuiltinTy {
        let mut holds = BuiltinTy::NonFalsyString;
        for &element in written {
            let ExprData::Literal {
                kind: LiteralKind::String,
                range,
            } = hir.expr(element)
            else {
                return BuiltinTy::String;
            };
            let Some((one, exact)) = self.quoted_string_holds(*range) else {
                return BuiltinTy::String;
            };
            let spells_a_number = !exact
                || self
                    .text_in(*range)
                    .map(|text| text.trim_matches(['\'', '"']).trim())
                    .is_some_and(|text| text.parse::<f64>().is_ok());
            let one = match spells_a_number {
                true => one.string_join(BuiltinTy::NonEmptyString).unwrap_or(one),
                false => one,
            };
            holds = holds.string_join(one).unwrap_or(holds);
        }
        holds
    }

    /// What a quoted string literal holds, read off its text, and whether that reading is the whole
    /// truth or only a lower bound.
    ///
    /// A single-quoted string is read exactly: its only escapes are `\\` and `\'`, each of which is a
    /// character, so it is empty or `'0'` exactly when its text is. A double-quoted one is exact only
    /// without a backslash -- an escape is at least one character and may be any of them, `"\060"`
    /// is `'0'` -- so one with an escape is known to hold something and nothing more. `None` for a
    /// literal that is not quoted at all: a magic constant, which may be empty outside the scope it
    /// names.
    pub(crate) fn quoted_string_holds(&self, range: TextRange) -> Option<(BuiltinTy, bool)> {
        let text = self.text_in(range)?;
        if let Some(inner) = text
            .strip_prefix('\'')
            .and_then(|rest| rest.strip_suffix('\''))
        {
            return Some((BuiltinTy::string_holding(inner.as_bytes()), true));
        }
        let inner = text
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))?;
        Some(match inner.contains('\\') {
            true => (BuiltinTy::NonEmptyString, false),
            false => (BuiltinTy::string_holding(inner.as_bytes()), true),
        })
    }
}
