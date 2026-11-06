//! What a value is once an identity found it is not a given array.
//!
//! `if ($row === [null]) { return; }` says nothing about null on the path past it -- `$row` is still
//! an array -- but it says the array is not *that* one. PHP's `===` on arrays compares every slot,
//! keys and order included, so a value differing from the literal differs in at least one slot; and
//! where every slot but one is already the literal's value, the one left is the one that differs.
//! That is the whole rule, and it is all this file holds: anything less certain rules nothing out.

use phrost_hir::{
    ExprId, FileHir,
    body::{ArrayElementKind, ExprData, LiteralKind},
};
use phrost_model::{BuiltinTy, Name, TextSize};

use crate::{SentinelReach, resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// `held` with what an identity against `literal` failing rules out of it taken off.
    ///
    /// Only a [`Held::Record`] is read, because only a record states every key it holds: a list, a
    /// map or a shape that may hold more keys than it names differs from the literal by a key it
    /// never mentioned as readily as by a value. The keys have to be the literal's, in the
    /// literal's order, for the slots to be compared at all -- otherwise the two are not identical
    /// whatever they hold, and the failed test was always going to fail.
    ///
    /// Then every slot but one must hold *only* the literal's value there, which this can say for
    /// the three literals that are a type of their own -- `null`, `true` and `false`. The one left
    /// loses that value: `array{0: ?string}` against `[null]` is `array{0: string}`. A string or a
    /// number in the literal names one value of a type with many, and a slot holding it is neither
    /// fixed nor narrowed by this, so it settles nothing either way.
    pub(crate) fn not_identical_to(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: Held,
        literal: ExprId,
    ) -> Held {
        let Held::Record(fields) = &held else {
            return held;
        };
        let Some(entries) = Self::literal_entries(hir, literal) else {
            return held;
        };
        if entries.len() != fields.len()
            || entries
                .iter()
                .zip(fields.iter())
                .any(|((key, _), (name, _))| key != name)
        {
            return held;
        }
        let mut differs = None;
        for (index, ((_, kind), (_, field))) in entries.iter().zip(fields.iter()).enumerate() {
            if self.holds_only(hir, field, *kind) {
                continue;
            }
            if differs.replace(index).is_some() {
                return held;
            }
        }
        // Every slot already the literal's means the value *is* the literal, and the branch does
        // not run -- a guard proving a value cannot exist is not something to answer a type with.
        let Some(index) = differs else {
            return held;
        };
        let (_, field) = &fields[index];
        let narrowed = match entries[index].1 {
            LiteralKind::Null => match self.without_null(field.clone()) {
                Some(narrowed) => narrowed,
                None => return held,
            },
            LiteralKind::True => {
                self.of_kind(offset, hir, field.clone(), BuiltinTy::True, false, true)
            }
            LiteralKind::False => {
                self.of_kind(offset, hir, field.clone(), BuiltinTy::False, false, true)
            }
            LiteralKind::Int | LiteralKind::Float | LiteralKind::String => return held,
        };
        let mut fields = fields.to_vec();
        fields[index].1 = narrowed;
        Held::Record(fields.into_boxed_slice())
    }

    /// Whether a slot can hold nothing but the value a literal of `kind` is.
    fn holds_only(&self, hir: &FileHir, field: &Held, kind: LiteralKind) -> bool {
        match kind {
            LiteralKind::Null => self.null_reach_of(hir, field) == SentinelReach::Certain,
            LiteralKind::False => self.false_reach_of(hir, field) == SentinelReach::Certain,
            LiteralKind::True => {
                self.kind_of_held(hir, field) == Some(BuiltinTy::True)
                    && self.null_reach_of(hir, field) == SentinelReach::Impossible
            }
            LiteralKind::Int | LiteralKind::Float | LiteralKind::String => false,
        }
    }

    /// The literal's slots in order, each with its key and the kind of value written there.
    ///
    /// Keyed the way PHP keys it: a bare entry takes the next integer, and an integer key written
    /// out moves the counter past it -- the rule `record_literal_fields` applies to the value side,
    /// so the two key the same literal the same way. `None` for anything the body layer would not
    /// have handed over, which it checked: every entry a literal, every key one too.
    fn literal_entries(hir: &FileHir, literal: ExprId) -> Option<Vec<(Name, LiteralKind)>> {
        let ExprData::ArrayLiteral { elements, .. } = hir.expr(literal) else {
            return None;
        };
        let mut next = 0_usize;
        let mut entries = Vec::new();
        for &element in hir.expr_list(*elements) {
            let (key, value) = match hir.expr(element) {
                ExprData::ArrayElement {
                    kind: ArrayElementKind::LiteralKey { name, .. },
                    value,
                    ..
                } => {
                    if let Ok(written) = name.as_str().parse::<usize>() {
                        next = written.saturating_add(1);
                    }
                    (name.clone(), *value)
                }
                ExprData::ArrayElement { .. } => return None,
                _ => {
                    let key = phrost_model::slot_name(next)?;
                    next += 1;
                    (key, element)
                }
            };
            let ExprData::Literal { kind, .. } = hir.expr(value) else {
                return None;
            };
            entries.push((key, *kind));
        }
        Some(entries)
    }
}
