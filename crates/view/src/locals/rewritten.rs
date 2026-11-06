//! What a container holds after a loop wrote into it through a reference.
//!
//! `foreach ($ints as &$item) { $item = 'x'; }` leaves `$ints` holding what it held or what was
//! written -- a union, because the list may be empty and a pass may write nothing -- and the body
//! layer hands over both halves without knowing which writes change anything. That is a question
//! about types, and it is the whole of this file: a write the element already admits changes
//! nothing, and the container is then handed back exactly as it was declared.

use phrost_body::OperandSite;
use phrost_hir::FileHir;
use phrost_model::{BuiltinTy, TextSize};
use phrost_ty::compat::Fit;

use crate::{SentinelReach, resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// `before` with every element widened by what the loop wrote, where something wrote a type
    /// the element does not already admit.
    ///
    /// Only an array: an object iterated by reference walks its properties, and a container this
    /// cannot read the elements of -- a bare `array` -- holds anything already. A null the
    /// container may be stays: a loop over null runs no pass and writes nothing.
    ///
    /// A write this cannot read leaves the container an `array` with nothing said of its elements,
    /// which is the claim an element write makes when nothing else does -- not the declared element
    /// type, which the write may have replaced, and not no type at all, which would forget it is an
    /// array.
    ///
    /// The keys are the container's own, since a write through a reference replaces a value and
    /// never a key: a stated key type travels, and a container that stated none is rebuilt as the
    /// keyless `V[]` it was written as. A record keeps its keys too, each slot widened in place.
    pub(crate) fn rewritten_elements(
        &self,
        offset: TextSize,
        hir: &FileHir,
        before: Held,
        at: TextSize,
        writes: Option<&[OperandSite]>,
        budget: u8,
    ) -> Held {
        if self.kind_of_held(hir, &before) != Some(BuiltinTy::Array) {
            return before;
        }
        let nullable = self.null_reach_of(hir, &before) != SentinelReach::Impossible;
        let Some(inner) = self.without_null(before.clone()) else {
            return before;
        };
        let wrap = |held: Held| match nullable {
            true => Self::nullable(held),
            false => held,
        };
        let Some(writes) = writes else {
            return wrap(Held::Builtin(BuiltinTy::Array));
        };
        let mut written = Vec::with_capacity(writes.len());
        for write in writes {
            match self.resolve_path(write.at, hir, &write.value, budget) {
                Some(held) => written.push(held),
                None => return wrap(Held::Builtin(BuiltinTy::Array)),
            }
        }

        if let Held::Record(fields) = &inner {
            let mut changed = false;
            let fields = fields
                .iter()
                .map(|(key, field)| {
                    let field = match self.widened(offset, hir, field, &written) {
                        Some(widened) => {
                            changed = true;
                            widened
                        }
                        None => field.clone(),
                    };
                    (key.clone(), field)
                })
                .collect::<Vec<_>>();
            return match changed {
                true => wrap(Held::Record(fields.into_boxed_slice())),
                false => before,
            };
        }

        let Some(element) = self.element_of_held(at, hir, &inner) else {
            return before;
        };
        let Some(value) = self.widened(offset, hir, &element, &written) else {
            return before;
        };
        wrap(match self.iteration_key_of_held(at, hir, &inner) {
            Some(key) => Held::MapOf {
                key: Box::new(key),
                value: Box::new(value),
            },
            None => Held::ListOf(Box::new(value)),
        })
    }

    /// `element` joined with the writes it does not already admit, or `None` when it admits all of
    /// them -- which is what keeps `$item = $item * 2` over a `list<int>` from rebuilding a
    /// container that did not change.
    fn widened(
        &self,
        offset: TextSize,
        hir: &FileHir,
        element: &Held,
        written: &[Held],
    ) -> Option<Held> {
        let wider = written
            .iter()
            .filter(|write| self.fit_held_strictly(offset, hir, write, element) != Fit::Accepted)
            .cloned()
            .collect::<Vec<_>>();
        if wider.is_empty() {
            return None;
        }
        self.union_of(
            offset,
            hir,
            std::iter::once(element.clone()).chain(wider).collect(),
            phrost_body::Unless::Nothing,
        )
    }
}
