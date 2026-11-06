//! Which call produced the value a step is reached through.
//!
//! The lookback a forwarded catalog binding needs, and nothing else asks for: some APIs put the one
//! word that says what comes back on the *previous* link of a chain, or a whole statement away in
//! the assignment that made the receiver. Kept out of [`super::steps`] because it is a walk
//! backwards over a path that has already been resolved, not a rule for what a step holds.

use phrost_body::{PathRoot, PathStep};
use phrost_hir::FileHir;
use phrost_model::{Name, TextSize};

use crate::view::{FOLLOW_LIMIT, IndexView};

impl IndexView<'_> {
    /// The call that produced what the step at `index` is called on, when it passed a literal.
    ///
    /// Some APIs put the one word that says what comes back on the *previous* link of the chain --
    /// see [`returned_through`](IndexView::returned_through). That link is the step before, or, for
    /// the first step, whatever the root itself came from: `$storage = $manager->getStorage('node')`
    /// puts the id a whole statement away, and that assignment is the commoner half of how the
    /// idiom is written.
    ///
    /// The names are cloned, which is one reference count each: this is asked only for a method a
    /// framework contracted, so it is not on the path every member access walks.
    pub(crate) fn producing_call(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        index: usize,
    ) -> Option<(Name, Name)> {
        match index.checked_sub(1) {
            Some(before) => Self::literal_call(&path.steps[before]),
            None => self.producing_call_of_root(offset, hir, &path.root, FOLLOW_LIMIT),
        }
    }

    /// The same, for a value that a root stands for rather than a step.
    ///
    /// Recursive through the roots that wrap another path, because a guard is written between the
    /// assignment and the call as often as not: `if ($storage) { $storage->load($id); }` puts a
    /// `NotNull` in the way and the id is still the only thing that names the entity.
    fn producing_call_of_root(
        &self,
        offset: TextSize,
        hir: &FileHir,
        root: &PathRoot,
        budget: u8,
    ) -> Option<(Name, Name)> {
        let budget = budget.checked_sub(1)?;
        // Each wrapper says where its subject is to be resolved, and using the wrong position is
        // what makes this loop: asking for `$storage` *inside* the guard finds the guard again.
        let (inner, offset) = match root {
            PathRoot::Local(name) => {
                let bound = phrost_body::local_binding(
                    hir,
                    offset,
                    name.as_str(),
                    &|hir, expr| self.call_never_returns(offset, hir, expr),
                    &self.asserting(),
                )?;
                let at = bound.written_at.unwrap_or(offset);
                (std::borrow::Cow::Owned(bound.value), at)
            }
            PathRoot::NotNull { of, at, .. }
            | PathRoot::NotEmptyString { of, at, .. }
            | PathRoot::Null { of, at, .. } => (std::borrow::Cow::Borrowed(&**of), *at),
            // `$c ? $this->getStorage('node') : NULL`. One operand carrying the whole value is
            // what a catalog contract is about; more than one real operand are that many values,
            // and none of them produced any other.
            PathRoot::EitherOf { operands, .. } => {
                let mut real = operands
                    .iter()
                    .filter(|operand| !operand.value.names_null());
                let only = real.next()?;
                if real.next().is_some() {
                    return None;
                }
                (std::borrow::Cow::Borrowed(&only.value), only.at)
            }
            PathRoot::Asserted { receiver, at, .. } => {
                (std::borrow::Cow::Borrowed(&**receiver), *at)
            }
            _ => return None,
        };

        match inner.steps.last() {
            Some(step) => Self::literal_call(step),
            None => self.producing_call_of_root(offset, hir, &inner.root, budget),
        }
    }

    /// The method and the *first* literal argument of a call step, when it is one and has one.
    ///
    /// The first, because that is what a catalog contract means by "the argument":
    /// `getStorage('node')` names the entity type there and nowhere else.
    fn literal_call(step: &PathStep) -> Option<(Name, Name)> {
        match step {
            PathStep::MethodReturn {
                name, arguments, ..
            } => Some((
                name.clone(),
                arguments
                    .iter()
                    .find(|argument| argument.position == 0)?
                    .value
                    .clone(),
            )),
            _ => None,
        }
    }
}
