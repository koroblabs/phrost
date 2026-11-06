//! Taking a union apart, and putting back what a guard left.
//!
//! The one place a held type is *rebuilt* rather than read: `is_string($x)` and
//! `$x instanceof Foo` remove operands, and what remains has to be expressible without a type
//! table open for writing. Separate from [`super::class_of`] because that one reduces a union to
//! the class its operands share and this one keeps the operands apart -- the two answers to a
//! union are opposite, and a caller asks for exactly one of them.

use phrost_hir::{FileHir, TypeRefData};
use phrost_model::TextSize;

use crate::{
    resolve::{Held, UnionOperand},
    view::IndexView,
};

impl IndexView<'_> {
    /// A union taken apart, as much of each operand as a narrowing test needs.
    ///
    /// `None` for anything that is not a union, which is the signal that there is nothing to narrow
    /// rather than that narrowing found nothing.
    ///
    /// Both spellings, because both reach here: a parameter's `Foo|string` is written in the file
    /// being read, and a function's `Foo|Bar` return comes back interned from another one.
    pub(crate) fn union_operands(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
    ) -> Option<Vec<UnionOperand>> {
        match held {
            Held::Object(_) => None,
            Held::Nullable(inner) => self.union_operands(offset, hir, inner),
            Held::Record(_) => None,
            Held::Written(id) => {
                let TypeRefData::Union(parts) = hir.type_ref(*id) else {
                    return None;
                };
                Some(
                    hir.type_ref_list(*parts)
                        .to_vec()
                        .iter()
                        .map(|part| UnionOperand {
                            held: Held::Written(*part),
                            class: self.class_of_type_ref(offset, hir, *part),
                            kind: Self::written_kind(hir, *part),
                        })
                        .collect(),
                )
            }
            Held::Stored { ty, scope } => {
                // Under an interned `?`, which is where `A|B|null` puts its union: the null is
                // lifted to a wrapper at interning, so a reader matching only `TypeRef::Union`
                // finds a nullable and stops. Taking it off is what the `Held::Nullable` arm above
                // already does for the built spelling of the same value -- whether null can arrive
                // is a different question from what the value is one of, and the null vocabulary
                // owns it.
                let ty = match self.skeleton.types().get(*ty) {
                    phrost_ty::TypeRef::Nullable(inner) => *inner,
                    _ => *ty,
                };
                let phrost_ty::TypeRef::Union(parts) = self.skeleton.types().get(ty) else {
                    return None;
                };
                Some(
                    self.skeleton
                        .types()
                        .operands(*parts)
                        .to_vec()
                        .iter()
                        .map(|part| UnionOperand {
                            held: Held::Stored {
                                ty: *part,
                                scope: scope.clone(),
                            },
                            class: self.class_of_stored(*part, scope.as_ref()),
                            kind: self.stored_kind(*part),
                        })
                        .collect(),
                )
            }
            Held::Class(_)
            | Held::Builtin(_)
            | Held::ListOf(_)
            | Held::MapOf { .. }
            | Held::Callable(_) => None,
            // Already taken apart. Described the same way, so a second narrowing composes with the
            // first -- which is what makes `if (is_string($x)) { return; } if ($x instanceof Foo)`
            // reach an answer at all.
            // Neither is a union, so there is nothing here to narrow: a guard about either leaves it
            // as it was.
            Held::AllOf(_) | Held::Applied { .. } => None,
            Held::OneOf(operands) => Some(
                operands
                    .iter()
                    .map(|operand| UnionOperand {
                        held: operand.clone(),
                        class: self.class_of_held(offset, hir, operand),
                        kind: self.kind_of_held(hir, operand),
                    })
                    .collect(),
            ),
        }
    }

    /// Whether a union operand is a union in its own right, and one this may take apart.
    ///
    /// Only the two interned spellings, and only where the union is the whole of what they hold: a
    /// `?T` hides its null in a wrapper that [`Self::union_operands`] drops on the way in, so
    /// rebuilding from what it returns would take the null off a value no guard said anything
    /// about. The union *containing* a null operand is fine -- that one comes back as an operand
    /// and goes back as one.
    pub(crate) fn union_inside(&self, held: &Held) -> bool {
        match held {
            Held::Stored { ty, .. } => {
                matches!(self.skeleton.types().get(*ty), phrost_ty::TypeRef::Union(_))
            }
            Held::Written(id) => self.current().is_some_and(|open| {
                matches!(open.hir().type_ref(*id), phrost_hir::TypeRefData::Union(_))
            }),
            _ => false,
        }
    }

    /// What a narrowing left of a union.
    ///
    /// One survivor is that operand, which keeps everything it carried -- arguments and all -- by
    /// pointing at it rather than rebuilding it. Several are a [`Held::OneOf`], which is the one
    /// shape here that is built: a written type lives in another file's arena and an interned one
    /// needs the table open for writing, so a union with an operand removed can be expressed no
    /// other way.
    ///
    /// Nothing left is the value as it was, which under a negative test means the test ruled
    /// nothing out. Under a positive one it is a contradiction, and the caller decides what to say
    /// about it -- see [`IndexView::of_kind`], where a guard that named a kind answers with the
    /// kind rather than with the value it just ruled out.
    ///
    /// `keep` answers what becomes of one operand: `None` drops it, and `Some` is what it becomes
    /// -- usually itself. Replacing rather than only filtering is what lets a guard *narrow* an
    /// operand instead of merely surviving it: `Base|false` under `instanceof Sub` has no operand
    /// that is a `Sub`, and a filter can only answer "keep nothing", which then leaves the union
    /// whole. See `of_one_of_classes`, where the three hierarchy relations live.
    pub(crate) fn narrowed_union(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: Held,
        keep: &dyn Fn(UnionOperand) -> Option<Held>,
    ) -> Held {
        let Some(operands) = self.union_operands(offset, hir, &held) else {
            return held;
        };
        let kept = operands
            .into_iter()
            .filter_map(|operand| match self.union_inside(&operand.held) {
                // An operand that is itself a union answers nothing about its own class or kind, so
                // judged whole it survives every guard intact -- `int|false` joined with an `int`
                // kept its `false` right through `if (!$timestamp)`, and the guard ran, returned a
                // type and changed nothing. Narrowed operand by operand instead, which is the same
                // claim applied to each alternative.
                true => Some(self.narrowed_union(offset, hir, operand.held, keep)),
                false => keep(operand),
            })
            .collect::<Vec<_>>();
        match kept.is_empty() {
            // Nothing survived, which means the branch cannot be entered with this value at all --
            // `if (!is_array($rows))` where `$rows` is an `array`. Handing back what it was says
            // the opposite of what the guard proved, and every check then judges the branch against
            // a type the guard has ruled out: a `?string` parameter given "an `array`" on a line
            // that cannot run. `mixed` is the honest answer, being the one type every check reads
            // as *no claim* -- this layer proves things about values and does not decide which
            // branches are reachable.
            true => Held::Builtin(phrost_model::BuiltinTy::Mixed),
            false => Self::one_of(kept),
        }
    }
}
