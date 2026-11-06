//! What a value found in a container is.
//!
//! `in_array($x, $list, true)` holding says `$x` is identical to one of `$list`'s elements, so it is
//! of their type -- which is a claim with two values in it, and what the second one holds is the
//! container's declaration's to say. Its own file because it is neither a guard about the value
//! alone ([`super::narrowing`]) nor a root that produces a value from nothing ([`super::path`]): it
//! meets two types, and the rules for meeting them are the whole of it.

use phrost_hir::FileHir;
use phrost_model::{BuiltinTy, TextSize};
use phrost_ty::compat::Fit;

use crate::{SentinelReach, resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// What a value is once it was found among a container's elements: the meet of what it was
    /// declared as and what they are.
    ///
    /// Three cases, and each is the narrowest thing the two types say together.
    ///
    /// - **The elements are narrower.** A `mixed` needle found in a `list<User>` is a `User`, and so
    ///   is a `?User` one: the needle *is* one of them, so it is what they are -- type arguments and
    ///   all, which is why the element itself is the answer rather than the class it names.
    /// - **Neither is.** What the needle may be is filtered by what the elements may be: a
    ///   `User|Admin|null` found in a `list<User>` keeps the `User`, and loses the `Admin` because no
    ///   value is both of two unrelated classes. An operand is kept unless the judgement can *prove*
    ///   it meets none of them, in either direction -- "reject only what can be proven wrong",
    ///   which is the rule [`crate::compat`] is built on, read here for overlap instead of fit.
    /// - **Null**, which goes whenever the elements cannot be null, whatever else the needle was: a
    ///   `?User` is nullable in a wrapper and is not a union [`Self::narrowed_union`] can take
    ///   apart, so the filter alone never reaches it.
    ///
    /// Either side unknown leaves the other: a needle nothing typed is whatever the elements are,
    /// and a container that says nothing about its elements says nothing about the needle. `mixed`
    /// elements say nothing either -- every value meets `mixed`.
    pub(crate) fn found_among(
        &self,
        offset: TextSize,
        hir: &FileHir,
        needle: Option<Held>,
        element: Option<Held>,
    ) -> Option<Held> {
        let Some(element) =
            element.filter(|element| self.kind_of_held(hir, element) != Some(BuiltinTy::Mixed))
        else {
            return needle;
        };
        let Some(needle) = needle else {
            return Some(element);
        };
        if self.fit_held_strictly(offset, hir, &element, &needle) == Fit::Accepted {
            return Some(element);
        }
        let needle = match self.null_reach_of(hir, &element) {
            SentinelReach::Impossible => match self.without_null(needle) {
                Some(needle) => needle,
                // A needle that could only be null, found among elements that cannot be: the branch
                // does not run, and the elements are the one answer that is not a contradiction.
                None => return Some(element),
            },
            _ => needle,
        };
        let overlaps = |operand: &Held| {
            self.fit_held_strictly(offset, hir, operand, &element) != Fit::Rejected
                || self.fit_held_strictly(offset, hir, &element, operand) != Fit::Rejected
        };
        Some(self.narrowed_union(offset, hir, needle, &|operand| {
            overlaps(&operand.held).then_some(operand.held)
        }))
    }
}
