//! Whether a call's argument is an array holding one key with one of a few literals.
//!
//! WordPress decides what `get_posts()`, `get_comments()`, `get_terms()` and a dozen more return by
//! the shape of their `$args`: `($args is array{ fields: 'ids', ... } ? int[] : WP_Post[])`. A call
//! settles it from the array it wrote, and nearly every one settles it the same way -- by leaving the
//! key out. So the test is read off the literal: written at the call, or assigned to the variable
//! the call passed, with what was written into it since.
//!
//! Anything this cannot see -- a spread, a computed key, a value that is not a literal, an array
//! built somewhere else -- settles nothing, and the declaration's own union stands.

use phrost_hir::{
    ExprId, FileHir,
    body::{ArrayElementKind, ExprData, LiteralKind},
};
use phrost_model::Name;
use phrost_sema::ShapeLiteral;

use super::CallFacts;
use crate::view::IndexView;

impl IndexView<'_> {
    /// Whether the array the call passed at `argument` holds `key` with one of `accepts` -- and, for
    /// a sealed shape, nothing else. `None` for an argument this cannot read as a literal.
    pub(super) fn shape_test_holds(
        &self,
        at: &CallFacts<'_>,
        written: ExprId,
        key: &Name,
        accepts: &[ShapeLiteral],
        sealed: bool,
    ) -> Option<bool> {
        let hir = at.hir;
        let (literal, fills) = match hir.expr(written) {
            // A string where an array is tested is not that array: WordPress takes a query string in
            // the same parameter, and the shape says what an *array* has to hold.
            ExprData::Literal { .. } => return Some(false),
            ExprData::ArrayLiteral { .. } => (written, Vec::new()),
            ExprData::Variable { name, .. } => self.literal_bound_to(at, name.as_str())?,
            _ => return None,
        };
        // What was written into the array since says nothing about the key unless it names that key
        // or a key nothing here can name -- `$args['s'] = $search` above `get_posts($args)` is the
        // ordinary shape, and leaves `fields` exactly as the literal did. Any of them is another key,
        // which a sealed shape does not admit.
        let mut others = false;
        for fill in &fills {
            match &fill.slot {
                phrost_body::FilledSlot::Named(name)
                | phrost_body::FilledSlot::Through(Some(name))
                    if name.as_str() != key.as_str() =>
                {
                    others = true;
                }
                phrost_body::FilledSlot::Appended => others = true,
                _ => return None,
            }
        }
        let ExprData::ArrayLiteral { elements, .. } = hir.expr(literal) else {
            return None;
        };
        let mut found = None;
        for &element in hir.expr_list(*elements) {
            match hir.expr(element) {
                ExprData::ArrayElement {
                    kind: ArrayElementKind::LiteralKey { name, .. },
                    value,
                    ..
                } => {
                    if name.as_str() == key.as_str() {
                        found = Some(*value);
                    } else {
                        others = true;
                    }
                }
                // A spread brings keys this cannot list, and a computed key may be the one tested.
                ExprData::ArrayElement { .. } => return None,
                // A bare entry takes an integer key, which a string key is never.
                _ => others = true,
            }
        }
        let Some(value) = found else {
            return Some(false);
        };
        if sealed && others {
            return Some(false);
        }
        let written = self.shape_literal(hir, value)?;
        Some(accepts.contains(&written))
    }

    /// The array literal a variable holds where the call reads it -- its one binding, an
    /// unconditional assignment of a literal -- and what has been written into it since.
    fn literal_bound_to(
        &self,
        at: &CallFacts<'_>,
        name: &str,
    ) -> Option<(ExprId, Vec<phrost_body::SlotFill>)> {
        let bound = phrost_body::local_binding(
            at.hir,
            at.offset,
            name,
            &|hir, expr| self.call_never_returns(at.offset, hir, expr),
            &self.asserting(),
        )?;
        let (phrost_body::PathRoot::ArrayLiteral(literal), true) =
            (&bound.value.root, bound.value.steps.is_empty())
        else {
            return None;
        };
        let since = bound.written_at?;
        let filled =
            phrost_body::slots_filled_since(at.hir, at.offset, name, since, &|hir, expr| {
                self.call_never_returns(at.offset, hir, expr)
            });
        Some((*literal, filled))
    }

    /// The literal a value was written as, in the vocabulary a shape test compares against.
    ///
    /// The text is read from the open file, because a literal's value is not kept once lowered --
    /// and a string holding an escape or an interpolation is not read at all, rather than read
    /// wrong.
    fn shape_literal(&self, hir: &FileHir, value: ExprId) -> Option<ShapeLiteral> {
        let ExprData::Literal { kind, range } = hir.expr(value) else {
            return None;
        };
        Some(match kind {
            LiteralKind::True => ShapeLiteral::True,
            LiteralKind::False => ShapeLiteral::False,
            LiteralKind::Null => ShapeLiteral::Null,
            LiteralKind::Int => ShapeLiteral::Int(self.text_in(*range)?.parse().ok()?),
            LiteralKind::String => {
                let text = self.text_in(*range)?;
                let inner = text
                    .strip_prefix('\'')
                    .and_then(|rest| rest.strip_suffix('\''))
                    .or_else(|| {
                        text.strip_prefix('"')
                            .and_then(|rest| rest.strip_suffix('"'))
                    })?;
                if inner.contains(['\\', '$']) {
                    return None;
                }
                ShapeLiteral::String(phrost_model::display_name(inner))
            }
            LiteralKind::Float => return None,
        })
    }
}
