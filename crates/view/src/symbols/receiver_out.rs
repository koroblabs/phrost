//! What a call leaves in the value it was made on: a method's `@phpstan-self-out`.
//!
//! The reading half of `Contract::ReceiverOut`. Each slot is a [`phrost_sema::ReturnBranch`], and
//! it is read by the reader a return's branches go through, so a template the receiver supplies, an
//! argument's type and a callback's result each mean here exactly what they mean in a `@return`.

use phrost_model::Name;

use super::CallFacts;
use crate::{
    resolve::{Held, RelativeScope},
    view::IndexView,
};

impl IndexView<'_> {
    /// What the receiver of `at`'s call holds once it returns, when `contract` says.
    ///
    /// `called` is the class the call was made on and `declaring` the one that wrote the
    /// declaration: `static` in the out type means the first and `self` the second, and the
    /// receiver's own arguments -- `at.receiver` -- reach a template named there through the
    /// `@extends` ladder between the two, as they do for a return.
    ///
    /// A slot nothing settles is the class's own template bound, for the reason
    /// [`Self::applied_instance`] gives: the declaration says the value is replaced, so what it
    /// held before is not an answer for that slot, and the bound is the most the declaration still
    /// says. `$items->transform(fn ($x) => $unknown)` is a collection of `mixed` afterwards.
    pub(crate) fn left_in_receiver(
        &self,
        contract: &phrost_sema::Contract,
        params: &[phrost_sema::ParamRow],
        called: Name,
        declaring: Name,
        at: &CallFacts<'_>,
    ) -> Option<Held> {
        let phrost_sema::Contract::ReceiverOut { class, arguments } = contract else {
            return None;
        };
        let scope = RelativeScope { called, declaring };
        let class = self
            .class_location(self.class_of_stored(*class, Some(&scope))?.as_str())?
            .fqn;
        let filled = arguments
            .iter()
            .map(|branch| self.held_of_branch(at, branch.clone(), params, Some(scope.clone())))
            .collect();
        self.applied_instance_of(class, filled)
    }
}
