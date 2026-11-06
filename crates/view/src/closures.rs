//! What a block closure with no declared return type gives back.
//!
//! `$make = function () { return $repo->find(1); }; $make()->title()` — three expressions that
//! answered nothing, while the same code written `fn () => $repo->find(1)` answered, and the same
//! code written `function (): Row` answered. The gap was the one shape in between: a body with
//! statements and no declaration.
//!
//! # Why this is decidable here and inferring a *named* function's return is not
//!
//! Both questions look the same and are not. A named function is called from another file, so
//! answering it means either storing an inferred type at ingest — where the callee may not be
//! indexed yet, and where the answer must survive the disk cache — or opening a second file at
//! request time, which is the shape this project refuses. A closure is assigned and invoked in one
//! body: the statements are in the same [`phrost_hir::FileHir`] the question is being asked
//! against, already lowered, whether that HIR came from an open buffer or from the scratch one a
//! closed-file request opens. So there is no ingest ordering, no cross-file read, and `open ==
//! closed` falls out rather than needing to be arranged.
//!
//! # Conservative or nothing
//!
//! Every `return` has to resolve or the answer is no type at all. Half a union is a claim the code
//! did not make: offering the members of the branch that did resolve offers a call that is a fatal
//! error on every run that took the other one. This is the same rule [`phrost_body::PathRoot`]'s
//! `EitherOf` states, and the join is the same one — [`IndexView::union_of`] — so a closure that
//! returns the one class down three branches holds that class rather than a union of three copies
//! of it.
//!
//! Three things contribute a type nobody wrote, and each is PHP's rule rather than a guess:
//!
//! - **A bare `return;` hands back null.** So does a path that reaches the end of the body without
//!   returning at all, which is what [`phrost_body::falls_off_end`] answers. Both are why the
//!   inferred type of a closure with a guard clause is `?Row` and not `Row` — and suppressing that
//!   null to keep a diagnostic quiet is exactly the leniency this project refuses.
//! - **A generator is not its returns.** `function () { yield 1; }` produces a `Generator`, and a
//!   `return` inside it hands its value to `Generator::getReturn()` rather than to the caller. So a
//!   body flagged [`phrost_hir::body::BodyData::is_generator`] is declined whole.
//! - **A body that returns nothing and cannot fall off its end** — every path throws — is `never`,
//!   which no reader here wants and none of them can use. Declined rather than invented.
//!
//! # Recursion
//!
//! `$fact = function (int $n) use (&$fact) { return $n <= 1 ? 1 : $n * $fact($n - 1); };` asks this
//! question about a closure while it is already being answered. The follow budget bounds it, but
//! only after re-entering several times over, and each level multiplies by the number of returns.
//! So re-entry is refused outright, on [`IndexView::inferring_closures`]: the inner ask answers
//! nothing, which makes the whole union nothing, which is the honest answer for a closure whose
//! type is defined in terms of itself.

use phrost_hir::{BodyId, ExprId};
use phrost_model::BuiltinTy;

use crate::{resolve::Held, symbols::CallFacts, view::IndexView};

impl IndexView<'_> {
    /// The union of every value a block closure's `return` statements hand back, or nothing.
    ///
    /// Each value is resolved at *its own* position rather than at the caller's, so a name written
    /// inside the closure resolves in the closure's scope — which is the only scope where a
    /// captured `use ($db)` means anything.
    pub(crate) fn inferred_closure_return(
        &self,
        at: &CallFacts<'_>,
        closure: ExprId,
        body: BodyId,
    ) -> Option<Held> {
        let hir = at.hir;
        if hir.body(body).is_generator {
            return None;
        }
        let budget = at.budget.checked_sub(1)?;
        let _guard = Reentry::enter(self, closure)?;

        let statements = hir.body(body).statements;
        let returns = phrost_body::returned_values(hir, statements);

        // With no `: never` knowledge, which is the cheap answer and the one this can afford: the
        // expensive one is a full resolution per branch, and here a missed `: never` call only adds
        // a null the body cannot actually produce. Adding one is the direction that reports rather
        // than the direction that hides.
        let implicit_null = phrost_body::falls_off_end(hir, statements, &|_, _| false);

        let mut resolved = Vec::with_capacity(returns.len() + usize::from(implicit_null));
        for value in returns {
            match value {
                Some(value) => {
                    let path = phrost_body::value_path_of(hir, value)?;
                    let inside = hir.expr(value).range().start();
                    resolved.push(self.resolve_path(inside, hir, &path, budget)?);
                }
                None => resolved.push(Held::Builtin(BuiltinTy::Null)),
            }
        }
        if implicit_null {
            resolved.push(Held::Builtin(BuiltinTy::Null));
        }
        if resolved.is_empty() {
            return None;
        }

        // At the closure's own position for the same reason each operand was resolved at its: a
        // union folds by asking what its operands name, and a name written in this file is read
        // against this file's imports.
        self.union_of(
            hir.expr(closure).range().start(),
            hir,
            resolved,
            phrost_body::Unless::Nothing,
        )
    }
}

/// One closure's place on the stack of closures whose returns are being inferred.
///
/// A guard type rather than a pair of calls because every early exit above has to pop it, and a
/// `?` that forgot to would leave a closure permanently un-inferable for the rest of the request.
struct Reentry<'v, 'i> {
    view: &'v IndexView<'i>,
}

impl<'v, 'i> Reentry<'v, 'i> {
    /// Claims `closure`, or nothing when it is already being inferred further up the stack.
    fn enter(view: &'v IndexView<'i>, closure: ExprId) -> Option<Self> {
        let mut stack = view.inferring_closures.borrow_mut();
        if stack.contains(&closure) {
            return None;
        }
        stack.push(closure);
        Some(Self { view })
    }
}

impl Drop for Reentry<'_, '_> {
    fn drop(&mut self) {
        self.view.inferring_closures.borrow_mut().pop();
    }
}
