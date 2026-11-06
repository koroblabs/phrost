//! Whether a write reaches a position, and how surely.
//!
//! A body is not a list of statements, and reading it as one is what lets a claim leak between
//! branches: `if ($c) { $v = trim($v); } else { $v-> }` writes `$v` on a path the cursor is not on,
//! and the nearest write before the cursor is that one. So this is the walk that asks the question
//! the position walk cannot -- not *is the write earlier*, but *did control have to go through it*.
//!
//! The walk below is the statement of the rule and the oracle the tests compare against; what runs
//! on a request is `facts.rs`, which derives the tree's half of the same decision once per file and
//! applies the position's half per write. A rule changed here has to be changed there too, and the
//! oracle test is what says so.
//!
//! Structural, not a control-flow graph. Every answer is one of two, and both are safe to act on:
//! a write is [`Reach::Certain`] only when every branch holding it also holds the position, and
//! [`Reach::Possible`] otherwise. A write control cannot get here from at all is not handed over,
//! and there are two ways to be sure of that: the branch never comes back, or the position is in a
//! branch of the same construct and one construct takes one branch.

#[cfg(test)]
use lspkit_arena::IdSlice;
#[cfg(test)]
use phrost_hir::{ExprId, body::ExprData};
use phrost_hir::{FileHir, StmtId, body::StmtData};
#[cfg(test)]
use phrost_model::TextRange;
use phrost_model::TextSize;

use crate::walk::branch_range;
#[cfg(test)]
use crate::walk::{NeverReturns, leaves, visit_expression};

/// How surely a write that is written before a position ran before control got there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Reach {
    /// Every branch the write is written in also holds the position, so control came through it.
    Certain,
    /// The write is in a branch the position is not in. Control can still get from there to here,
    /// but it need not have gone that way -- so the write may have happened and may not.
    ///
    /// The position carried is where the *outermost* such branch begins, which is the last point at
    /// which none of this had happened. That is where the other half of the answer is read: what
    /// the name holds if the branch was skipped is what it held before the whole construct, and
    /// reading it any later reads it under the branch's own guard -- `if (!isset($x)) { $x = new
    /// Formatter(); }` would answer that `$x` was null, which is true only on the path that then
    /// wrote it.
    Possible(TextSize),
}

impl Reach {
    /// The same reach, for something written inside a branch beginning at `from`.
    pub(crate) fn inside(self, from: TextSize) -> Self {
        match self {
            // Already in a branch, and the outermost one is the one to read before: a nested branch
            // begins after its parent, where the parent's guard already holds.
            already @ Self::Possible(_) => already,
            Self::Certain => Self::Possible(from),
        }
    }
}

/// A branch of a construct, and what the position being elsewhere in that construct means.
#[cfg(test)]
#[derive(Clone, Copy)]
struct Sibling {
    /// The whole construct. Its start is where the value before any of its branches is read.
    construct: TextRange,
    /// Whether taking this branch rules the others out.
    ///
    /// True of an `if`, whose condition is evaluated once and chooses one side. False of a loop
    /// body, which runs before its own header on the pass after, and of a `catch`, which a
    /// `finally` in the same statement runs after.
    exclusive: bool,
}

#[cfg(test)]
impl Sibling {
    fn exclusive(construct: TextRange) -> Self {
        Self {
            construct,
            exclusive: true,
        }
    }

    fn shared(construct: TextRange) -> Self {
        Self {
            construct,
            exclusive: false,
        }
    }

    /// Whether a position outside this branch was reached without it.
    fn rules_out(self, offset: TextSize) -> bool {
        self.exclusive && self.construct.contains_inclusive(offset)
    }
}

/// Visits every expression control can reach `offset` from, saying how surely it ran.
///
/// The reach-aware counterpart of [`visit_statements`](crate::walk::visit_statements), and the same
/// coverage: closure bodies are their own scope and are not descended into, and an unmodelled shape
/// keeps its subtree. Expressions written where control cannot come from -- a branch ending in
/// `return`, `throw` or `exit`, or a branch of a construct whose other branch holds the position --
/// are not handed over at all, because nothing written there can be in effect here.
#[cfg(test)]
pub(crate) fn visit_reaching_expressions(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    offset: TextSize,
    never: NeverReturns<'_>,
    visit: &mut impl FnMut(&FileHir, ExprId, Reach),
) {
    reaching_statements(hir, statements, offset, never, Reach::Certain, visit);
}

#[cfg(test)]
fn reaching_statements(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    offset: TextSize,
    never: NeverReturns<'_>,
    reach: Reach,
    visit: &mut impl FnMut(&FileHir, ExprId, Reach),
) {
    for &id in hir.stmt_list(statements) {
        let data = hir.stmt(id);
        let construct = data.range();
        let from = construct.start();

        match data {
            StmtData::Expr { expr, .. } => reaching_expression(hir, *expr, offset, reach, visit),
            StmtData::Return { value, .. } => {
                if let Some(value) = value {
                    reaching_expression(hir, *value, offset, reach, visit);
                }
            }
            StmtData::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                // The condition is evaluated whenever the statement is: it is what chooses a
                // branch, so it is not in one.
                reaching_expression(hir, *condition, offset, reach, visit);
                for branch in [then_branch, else_branch] {
                    let sibling = Sibling::exclusive(construct);
                    reaching_branch(hir, *branch, offset, never, reach, sibling, visit);
                }
            }
            StmtData::Foreach {
                subject,
                key,
                value,
                body,
                ..
            } => {
                reaching_expression(hir, *subject, offset, reach, visit);
                // The targets are written by the header, which binds them once per element -- so
                // they are as conditional as the body they were written for.
                let inside = branch_reach(hir, *body, offset, reach, from);
                if let Some(key) = key {
                    reaching_expression(hir, *key, offset, inside, visit);
                }
                reaching_expression(hir, *value, offset, inside, visit);
                reaching_branch(
                    hir,
                    *body,
                    offset,
                    never,
                    reach,
                    Sibling::shared(construct),
                    visit,
                );
            }
            StmtData::While {
                condition, body, ..
            } => {
                reaching_expression(hir, *condition, offset, reach, visit);
                reaching_branch(
                    hir,
                    *body,
                    offset,
                    never,
                    reach,
                    Sibling::shared(construct),
                    visit,
                );
            }
            // `for ($i = 0; ...)` runs its init once and unconditionally; the condition and the
            // step run per pass, which makes them as conditional as the body.
            StmtData::For {
                init,
                conditions,
                step,
                body,
                ..
            } => {
                for &id in hir.expr_list(*init) {
                    reaching_expression(hir, id, offset, reach, visit);
                }
                let inside = branch_reach(hir, *body, offset, reach, from);
                for list in [conditions, step] {
                    for &id in hir.expr_list(*list) {
                        reaching_expression(hir, id, offset, inside, visit);
                    }
                }
                reaching_branch(
                    hir,
                    *body,
                    offset,
                    never,
                    reach,
                    Sibling::shared(construct),
                    visit,
                );
            }
            // A bare block chooses nothing: PHP has no block scope and no condition here.
            StmtData::Block { body, .. } => {
                reaching_statements(hir, *body, offset, never, reach, visit);
            }
            StmtData::Try {
                body,
                catches,
                finally,
                ..
            } => {
                // A `catch` is entered from a throw somewhere inside the `try`, which may be before
                // any given write in it -- so from a handler, the whole try body is conditional.
                // From anywhere else it is not: reaching past the statement means the body ran.
                let body_reach = match handles(hir, id, offset) {
                    true => reach.inside(from),
                    false => reach,
                };
                reaching_statements(hir, *body, offset, never, body_reach, visit);
                for &catch in hir.catch_list(*catches) {
                    // Not exclusive: a `finally` in the same statement runs after whichever catch
                    // ran, so a position in one is reached from any of them.
                    let sibling = Sibling::shared(construct);
                    reaching_branch(
                        hir,
                        hir.catch(catch).body,
                        offset,
                        never,
                        reach,
                        sibling,
                        visit,
                    );
                }
                // `finally` runs on every path out of the try, so it chooses nothing either.
                reaching_statements(hir, *finally, offset, never, reach, visit);
            }
            StmtData::Switch { subject, cases, .. } => {
                reaching_expression(hir, *subject, offset, reach, visit);
                reaching_cases(hir, *cases, offset, never, reach, construct, visit);
            }
            StmtData::Leave { .. } | StmtData::Annotation { .. } => {}
            StmtData::Bind { bindings, .. } => {
                for &id in hir.expr_list(*bindings) {
                    reaching_expression(hir, id, offset, reach, visit);
                }
            }
            StmtData::Unmodelled { inner, .. } => {
                for &id in hir.expr_list(*inner) {
                    reaching_expression(hir, id, offset, reach, visit);
                }
            }
        }
    }
}

/// The arms of a `switch`, which are exclusive only as far as they do not fall through.
///
/// PHP's `switch` enters one arm and runs on into the next unless something stops it, so an earlier
/// arm reaches a position in a later one exactly when every arm between them falls off its end.
/// That is the difference between `case 'a': $v = trim($v); break; case 'b': $v->` -- where the
/// write never happened -- and the same two arms without the `break`, where it always did.
#[cfg(test)]
fn reaching_cases(
    hir: &FileHir,
    cases: IdSlice<phrost_hir::SwitchCaseId>,
    offset: TextSize,
    never: NeverReturns<'_>,
    reach: Reach,
    construct: TextRange,
    visit: &mut impl FnMut(&FileHir, ExprId, Reach),
) {
    let list = hir.switch_case_list(cases);
    let holding = list.iter().position(|&case| {
        branch_range(hir, hir.switch_case(case).body).is_some_and(|r| r.contains_inclusive(offset))
    });

    for (index, &case) in list.iter().enumerate() {
        let case = hir.switch_case(case);
        // An arm's own conditions are compared against the subject in order, so they are evaluated
        // without the arm being entered.
        for &id in hir.expr_list(case.conditions) {
            reaching_expression(hir, id, offset, reach, visit);
        }

        // `true`: a `break` ends the arm, and ending the arm is exactly what stops the
        // fall-through being asked about here.
        let reached = match holding {
            Some(holding) if index < holding => list[index..holding]
                .iter()
                .all(|&between| !leaves(hir, hir.switch_case(between).body, true, never)),
            // This arm, a later one, or a position outside the arms altogether -- none of which
            // this rule settles.
            Some(_) | None => true,
        };
        let sibling = match reached {
            true => Sibling::shared(construct),
            false => Sibling::exclusive(construct),
        };
        reaching_branch(hir, case.body, offset, never, reach, sibling, visit);
    }
}

/// Descends into a branch, with what being in it means for anything written there.
///
/// Three ways not to descend, and the one that is easy to miss is the last: a branch ending in
/// `return`, `throw` or `exit` never comes back, so nothing written in it is in effect after it.
/// That is the same rule an early-return guard already relies on to narrow the code below it.
///
/// `break` and `continue` are deliberately not counted as leaving *here*. Whether either escapes
/// the region being asked about depends on where the position is -- a `break` in a `switch` arm
/// lands just after the `switch`, and one in a loop body lands after the loop -- so the one place
/// it settles anything is between the arms of a `switch`, where [`reaching_cases`] asks for it by
/// name.
///
/// A call to something declared `: never` is asked of `never`, the reader's oracle. This walk once
/// refused the question on cost -- each answer was a full resolution at the whole follow limit, and
/// asking it here cost 38 ms of the 30 ms diagnostics budget -- and that reasoning has expired
/// twice over: the oracle now rejects nearly every call by *name* before resolving anything, and
/// what runs on a request is `facts.rs`, which settles the tree's half once per revision and puts
/// only the unsettled case to the oracle. A branch that ends in `abort()` drops its writes exactly
/// as one ending in `return` does, which is what lets `$x = new Bar(); abort();` in an `if` leave
/// `$x` what it was before.
#[cfg(test)]
fn reaching_branch(
    hir: &FileHir,
    branch: IdSlice<StmtId>,
    offset: TextSize,
    never: NeverReturns<'_>,
    outer: Reach,
    sibling: Sibling,
    visit: &mut impl FnMut(&FileHir, ExprId, Reach),
) {
    let Some(range) = branch_range(hir, branch) else {
        return;
    };
    if range.contains_inclusive(offset) {
        // The position is in this branch, so being in it says nothing extra: whatever the code
        // around the branch already was, this is.
        return reaching_statements(hir, branch, offset, never, outer, visit);
    }
    if sibling.rules_out(offset) || leaves(hir, branch, false, never) {
        return;
    }
    reaching_statements(
        hir,
        branch,
        offset,
        never,
        outer.inside(sibling.construct.start()),
        visit,
    );
}

/// What being inside `branch` means for a position, without descending into it.
#[cfg(test)]
fn branch_reach(
    hir: &FileHir,
    branch: IdSlice<StmtId>,
    offset: TextSize,
    outer: Reach,
    from: TextSize,
) -> Reach {
    match branch_range(hir, branch).is_some_and(|range| range.contains_inclusive(offset)) {
        true => outer,
        false => outer.inside(from),
    }
}

/// Whether `offset` is in a `catch` or `finally` of this `try`.
pub(crate) fn handles(hir: &FileHir, id: StmtId, offset: TextSize) -> bool {
    let StmtData::Try {
        body,
        catches,
        finally,
        range,
    } = hir.stmt(id)
    else {
        return false;
    };
    if !range.contains_inclusive(offset) {
        return false;
    }
    // Not in the try body, and in one of the clauses that handles it. Asked that way round because
    // a `catch` clause's range covers its header as well as its body, and the `finally` keyword has
    // no node of its own.
    !branch_range(hir, *body).is_some_and(|range| range.contains_inclusive(offset))
        && (hir
            .catch_list(*catches)
            .iter()
            .any(|&catch| hir.catch(catch).range.contains_inclusive(offset))
            || branch_range(hir, *finally).is_some_and(|range| range.contains_inclusive(offset)))
}

/// Visits an expression tree, downgrading or dropping anything written in a branch of it.
///
/// A `match` arm, a ternary arm, and the right operand of `??` or `&&` are branches as surely as an
/// `if` is -- `match ($k) { 'a' => $v = trim($v), 'b' => $v-> }` is the same leak written as an
/// expression. They are found on the way rather than descended into by hand: [`visit_expression`]
/// hands over a parent before anything inside it, so recording a branch's span when it is reached
/// is enough to classify every write the span covers. Re-implementing that walk here would be a
/// second exhaustive match to keep in step with the first, which is the mistake its own comment
/// warns about.
#[cfg(test)]
fn reaching_expression(
    hir: &FileHir,
    id: ExprId,
    offset: TextSize,
    reach: Reach,
    visit: &mut impl FnMut(&FileHir, ExprId, Reach),
) {
    // Both empty for all but a handful of expressions, and `Vec::new` does not allocate until
    // pushed -- which matters, because this runs per statement on the request path.
    let mut branches: Vec<(TextRange, TextSize)> = Vec::new();
    let mut ruled_out: Vec<TextRange> = Vec::new();

    visit_expression(hir, id, &mut |hir, expr| {
        record_branches(hir, expr, offset, &mut branches, &mut ruled_out);
        // The overwhelming majority of statements hold no branching expression at all, and for
        // those there is nothing to compare a span against. Asked before the span is even read,
        // because this runs once per expression node of every body on every lookup.
        if branches.is_empty() && ruled_out.is_empty() {
            return visit(hir, expr, reach);
        }

        let range = hir.expr(expr).range();
        if ruled_out.iter().any(|branch| branch.contains_range(range)) {
            return;
        }
        // The first match is the outermost: a parent is recorded before anything inside it.
        let inside = branches
            .iter()
            .find(|(branch, _)| branch.contains_range(range))
            .map(|&(_, from)| from);
        visit(
            hir,
            expr,
            match inside {
                Some(from) => reach.inside(from),
                None => reach,
            },
        );
    });
}

/// Records the spans of `expr`'s branches that the position is not in.
///
/// Into `ruled_out` when the position is elsewhere in the same construct -- one `match` picks one
/// arm, and one ternary picks one side -- and into `branches` when the position is past the whole
/// thing, where the branch may or may not have been the one taken.
#[cfg(test)]
fn record_branches(
    hir: &FileHir,
    expr: ExprId,
    offset: TextSize,
    branches: &mut Vec<(TextRange, TextSize)>,
    ruled_out: &mut Vec<TextRange>,
) {
    let whole = hir.expr(expr).range();
    let elsewhere_within = whole.contains_inclusive(offset);
    let mut consider = |id: ExprId, exclusive: bool| {
        let range = hir.expr(id).range();
        if range.contains_inclusive(offset) {
            return;
        }
        match exclusive && elsewhere_within {
            true => ruled_out.push(range),
            false => branches.push((range, whole.start())),
        }
    };

    match hir.expr(expr) {
        ExprData::Match { arms, .. } => {
            for &arm in hir.match_arm_list(*arms) {
                consider(hir.match_arm(arm).body, true);
            }
        }
        ExprData::Conditional {
            then_branch,
            else_branch,
            ..
        } => {
            // `?:` has no then-branch: the value is the condition's own, which was evaluated
            // either way.
            if let Some(then_branch) = then_branch {
                consider(*then_branch, true);
            }
            consider(*else_branch, true);
        }
        // Short-circuit: the right operand runs only when the left decided it should. Not
        // exclusive -- the left is evaluated either way, so a position there is simply earlier.
        ExprData::Coalesce { right, .. } | ExprData::Logical { right, .. } => {
            consider(*right, false);
        }
        _ => {}
    }
}
