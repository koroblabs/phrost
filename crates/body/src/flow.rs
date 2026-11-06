//! Whether control can run past the end of a block, and what nothing can reach.
//!
//! Two questions, one walk, and opposite conservatism from [`leaves`](crate::walk::leaves) -- which
//! is why this is a module rather than another caller of it. `leaves` answers *does every path leave
//! here*, and it is conservative towards "no": a guard may narrow only what it has proven, and a
//! shape it cannot read must be assumed to fall through. Asking whether a function can fall off its
//! end needs the other direction. A `try`/`catch` where both halves return, and a `while (true)`
//! nothing breaks out of, are both shapes `leaves` calls "maybe" -- and reporting "not every path
//! returns" on a maybe would report every one of them in a project.
//!
//! So flow here is described by what leaves a block rather than by whether every path does, and the
//! constructs that *catch* a `break` are modelled, because that is the difference between a loop
//! that ends and one that does not.
//!
//! One walk answers both, and that is a cost decision as much as a tidiness one. The dead-code
//! question needs each statement's own verdict and then its children's; asking for them separately
//! walks every subtree once per level above it, and folding the two into [`Flow`] keeps it linear.
//!
//! Still not a control-flow graph. A condition is never evaluated, so `if (true)` chooses nothing
//! and an `if` with no `else` always falls through; the one literal that is read is the `true` of
//! `while (true)`, because a loop with no exit is the whole reason the question has an answer at
//! all.

use std::cmp::Reverse;

use lspkit_arena::IdSlice;
use phrost_hir::{
    CatchId, ExprId, FileHir, StmtId, SwitchCaseId,
    body::{ExprData, LeaveKind, LiteralKind, Passage, StmtData},
};
use phrost_model::TextRange;

use crate::walk::NeverReturns;

/// Whether control certainly can run past the last statement of `statements`.
///
/// What a declared return type asks of a body: PHP raises a `TypeError` on the run that falls off
/// the end of a function declared to hand something back, so an answer of `true` is a defect and an
/// answer of `false` is silence. *Certainly*, because a block ending in a statement this cannot read
/// answers `false` -- see [`Exits::guessed`].
///
/// `never` is the expensive half and the reason this takes it rather than deciding it -- see
/// [`NeverReturns`]. A caller on the request path should ask with `&|_, _| false` first and only pay
/// for a real answer about the handful of bodies that came back `true`: here, unlike everywhere else
/// in this crate, a missed `: never` call is a *false positive* rather than a miss, so the cheap
/// answer settles nothing on its own.
pub fn falls_off_end(hir: &FileHir, statements: IdSlice<StmtId>, never: NeverReturns<'_>) -> bool {
    let exits = Flow { never, dead: None }.list(hir, statements);
    exits.onward && !exits.guessed
}

/// Where a statement list -- and every body written inside it, closures included -- holds statements
/// no path reaches.
///
/// Ranges rather than ids, and one per run rather than one per statement: a `return` with four lines
/// under it is one mistake, and four findings would read as four.
///
/// Asked with no `: never` knowledge, deliberately. Here a missed one is a missed finding, which is
/// the direction this can afford -- and paying a full resolution per statement of every body is the
/// direction it cannot.
pub fn unreachable_regions(hir: &FileHir, statements: IdSlice<StmtId>) -> Vec<TextRange> {
    let mut out = Vec::new();
    collect_unreachable(hir, statements, &mut out);

    // Outermost first at each start, so a region contained in one already reported is dropped rather
    // than said twice -- which happens whenever a closure is written inside dead code.
    out.sort_by_key(|range| (range.start(), Reverse(range.end())));
    let mut covering: Option<TextRange> = None;
    out.retain(|&range| match covering {
        Some(outer) if outer.contains_range(range) => false,
        _ => {
            covering = Some(range);
            true
        }
    });
    out
}

/// The ways control can leave a block without leaving the function.
///
/// `return`, `throw`, `exit` and a `: never` call need no flag of their own: they leave the
/// function, and nothing enclosing them in one body treats them differently. What has to be tracked
/// is the two that land somewhere in the *same* body, because the construct that catches them
/// decides whether the code after it runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Exits {
    /// Control reaches the end of the block and runs on into whatever follows it.
    onward: bool,
    /// Some path takes a `break` this block does not consume.
    breaks: bool,
    /// Some path takes a `continue` this block does not consume.
    continues: bool,
    /// Whether [`Self::onward`] rests on a statement whose fate this cannot read.
    ///
    /// The block ends in a statement that leaves for somewhere this cannot follow: `goto`, which
    /// jumps to a label elsewhere, or `__halt_compiler()`. Both are unmodelled, and both are now
    /// told apart from an `echo` by [`Passage`], which lowering decides. The guess remains because
    /// where control went is still unknown -- only *that* it went is.
    ///
    /// Which of the two questions cares is the point of the flag. Dead code is reported off
    /// `onward == false`, and a guess is never that. Falling off the end is reported off `onward ==
    /// true`, so there the guess has to be dropped -- Symfony's `PhpArrayAdapter::get` ends its one
    /// `catch` in `goto get_from_pool`, which without this reads as a method that returns nothing.
    guessed: bool,
}

impl Exits {
    /// Control runs off the end and nowhere else -- every statement that is not a branch.
    const ONWARD: Self = Self {
        onward: true,
        breaks: false,
        continues: false,
        guessed: false,
    };

    /// Every path leaves the function here.
    const LEAVES: Self = Self {
        onward: false,
        breaks: false,
        continues: false,
        guessed: false,
    };

    /// Control runs off the end as far as this can tell, which is not far -- see [`Self::guessed`].
    const UNREADABLE: Self = Self {
        guessed: true,
        ..Self::ONWARD
    };

    /// Either of two paths out of one construct, so anything possible in one is possible.
    fn or(self, other: Self) -> Self {
        Self {
            onward: self.onward || other.onward,
            breaks: self.breaks || other.breaks,
            continues: self.continues || other.continues,
            guessed: self.guessed || other.guessed,
        }
    }

    /// Whether a `break` or a `continue` written here reaches past this block.
    fn escapes(self) -> bool {
        self.breaks || self.continues
    }
}

/// One walk over a body's statements, and what it is being asked for.
///
/// The `dead` list is what makes it two questions: with it, every statement list the walk enters
/// reports the run it holds that nothing reaches; without it, the walk only answers about the block
/// it was given. Both need each statement's verdict and its children's, which is one traversal.
struct Flow<'a> {
    never: NeverReturns<'a>,
    dead: Option<&'a mut Vec<TextRange>>,
}

impl Flow<'_> {
    /// The ways control leaves a statement list, run in order.
    ///
    /// A statement that never falls through ends the list: nothing after it runs, so nothing after
    /// it contributes a path out, and everything after it is the dead run. The `break`s and
    /// `continue`s met on the way do contribute, because each is a way out of this list that the
    /// statement after it never sees.
    fn list(&mut self, hir: &FileHir, statements: IdSlice<StmtId>) -> Exits {
        let list = hir.stmt_list(statements);
        let mut escaped = Exits::LEAVES;
        // The last statement's, because it is the one the list's own verdict rests on: an earlier
        // unreadable statement still had the one after it run.
        let mut guessed = false;

        for (index, &id) in list.iter().enumerate() {
            let exits = self.statement(hir, id);
            escaped.breaks |= exits.breaks;
            escaped.continues |= exits.continues;
            guessed = exits.guessed;
            if !exits.onward {
                // Not descended into: dead code inside dead code says nothing new.
                if let Some(dead) = self.dead.as_deref_mut() {
                    dead.extend(dead_run(hir, &list[index + 1..]));
                }
                return Exits {
                    onward: false,
                    guessed: false,
                    ..escaped
                };
            }
        }

        // Including an empty list, which is the shape of `function f(): string {}`.
        Exits {
            onward: true,
            guessed,
            ..escaped
        }
    }

    fn statement(&mut self, hir: &FileHir, id: StmtId) -> Exits {
        match hir.stmt(id) {
            StmtData::Return { .. } => Exits::LEAVES,
            StmtData::Leave { kind, .. } => match kind {
                LeaveKind::Break => Exits {
                    breaks: true,
                    ..Exits::LEAVES
                },
                LeaveKind::Continue => Exits {
                    continues: true,
                    ..Exits::LEAVES
                },
            },
            StmtData::Expr { expr, .. } => match self.expression_leaves(hir, *expr) {
                true => Exits::LEAVES,
                false => Exits::ONWARD,
            },
            StmtData::Block { body, .. } => self.list(hir, *body),
            // No condition is evaluated, so an `if` with no `else` -- an empty branch, which falls
            // through -- is the shape that keeps an early-return guard from reading as a defect.
            StmtData::If {
                then_branch,
                else_branch,
                ..
            } => {
                let taken = self.list(hir, *then_branch);
                taken.or(self.list(hir, *else_branch))
            }
            StmtData::Switch { cases, .. } => self.switch(hir, *cases),
            StmtData::While {
                condition, body, ..
            } => {
                let endless = is_literal_true(hir, *condition);
                Self::loop_exits(self.list(hir, *body), endless)
            }
            // `for (;;)` with no test is the other spelling of a loop that does not end on its own.
            StmtData::For {
                conditions, body, ..
            } => {
                let endless = hir.expr_list(*conditions).is_empty();
                Self::loop_exits(self.list(hir, *body), endless)
            }
            // A `foreach` over an empty subject runs its body no times at all.
            StmtData::Foreach { body, .. } => Self::loop_exits(self.list(hir, *body), false),
            StmtData::Try {
                body,
                catches,
                finally,
                ..
            } => self.try_statement(hir, *body, *catches, *finally),
            // A `static $x;` and a docblock note are statements control walks straight through, and
            // this can say so.
            StmtData::Bind { .. } | StmtData::Annotation { .. } => Exits::ONWARD,
            // An unmodelled statement still says what control does, because lowering classified it
            // while the node was in hand. Only the one that leaves for somewhere unfollowable is a
            // guess now; an `echo` walks through and says so.
            StmtData::Unmodelled { passage, .. } => match passage {
                Passage::Onward | Passage::Arrival => Exits::ONWARD,
                Passage::Elsewhere => Exits::UNREADABLE,
            },
        }
    }

    /// A `switch` consumes `break`, and PHP counts it as a loop for `continue` too -- a bare
    /// `continue` inside one behaves exactly like a `break`.
    ///
    /// Its arms fall through into each other, so only the *last* arm running off its end escapes the
    /// statement; an earlier one running off its end merely enters the next. And with no `default`,
    /// a subject matching no case walks straight past the whole thing, which is what makes a
    /// `switch` over an enum's every value still a fall-through.
    fn switch(&mut self, hir: &FileHir, cases: IdSlice<SwitchCaseId>) -> Exits {
        let mut has_default = false;
        let mut escaped = false;
        let mut guessed = false;
        let mut last_onward = true;

        for &id in hir.switch_case_list(cases) {
            let case = hir.switch_case(id);
            has_default |= case.is_default;
            let exits = self.list(hir, case.body);
            escaped |= exits.escapes();
            guessed |= exits.guessed;
            last_onward = exits.onward;
        }

        Exits {
            onward: !has_default || last_onward || escaped,
            breaks: false,
            continues: false,
            guessed,
        }
    }

    /// `finally` runs on every path out of the statement, so a `return` written in one settles the
    /// whole thing whatever the body and the handlers did.
    ///
    /// Otherwise control leaves normally when the body runs to its end or a handler does. A throw
    /// nothing catches leaves the *function*, which is not falling through -- so an uncaught one
    /// contributes no path here, and a `try` whose body and whose every `catch` return does not fall
    /// through.
    fn try_statement(
        &mut self,
        hir: &FileHir,
        body: IdSlice<StmtId>,
        catches: IdSlice<CatchId>,
        finally: IdSlice<StmtId>,
    ) -> Exits {
        // Walked first because it decides the answer, and walked whatever it decides because the
        // dead code in it is still dead code.
        let after = self.list(hir, finally);
        let mut exits = self.list(hir, body);
        for &id in hir.catch_list(catches) {
            exits = exits.or(self.list(hir, hir.catch(id).body));
        }

        if !after.onward {
            return after;
        }
        Exits {
            breaks: exits.breaks || after.breaks,
            continues: exits.continues || after.continues,
            guessed: exits.guessed || after.guessed,
            ..exits
        }
    }

    /// Whether a bare expression statement is one nothing runs after.
    ///
    /// `throw` and `exit` are syntax and are decided here. A call to something declared `: never` is
    /// not, and goes back to whoever can look the callee up.
    fn expression_leaves(&self, hir: &FileHir, id: ExprId) -> bool {
        match hir.expr(id) {
            ExprData::Throw { .. } | ExprData::Terminate { .. } => true,
            // A `match` written for its effect rather than its value. Every arm leaving is the only
            // way it ends the flow, and it needs a `default`: without one, a subject no arm covers
            // throws `UnhandledMatchError` -- which also leaves, but by a route this cannot tell
            // from a subject that matched.
            ExprData::Match { arms, .. } => {
                let list = hir.match_arm_list(*arms);
                !list.is_empty()
                    && list
                        .iter()
                        .any(|&arm| hir.expr_list(hir.match_arm(arm).conditions).is_empty())
                    && list
                        .iter()
                        .all(|&arm| self.expression_leaves(hir, hir.match_arm(arm).body))
            }
            _ => (self.never)(hir, id),
        }
    }

    /// A loop consumes both `break` and `continue`, so neither reaches past it.
    ///
    /// Which leaves one question: whether the loop ends at all. One that does falls through whatever
    /// its body says, because the test can be false the first time it is asked. One that does not
    /// falls through exactly when something inside it breaks out.
    ///
    /// `break 2;` is spelled the same as `break;` in this tree, so a labelled break out of a nested
    /// loop is consumed by the inner one and the outer one reads as endless. A miss, and the quiet
    /// direction.
    fn loop_exits(body: Exits, endless: bool) -> Exits {
        Exits {
            onward: !endless || body.breaks,
            breaks: false,
            continues: false,
            // A loop that ends on its own falls through whatever its body could not be read to say,
            // so only an endless one carries the guess out.
            guessed: endless && body.guessed,
        }
    }
}

/// `while (true)`, in the one place a literal condition is read at all.
fn is_literal_true(hir: &FileHir, id: ExprId) -> bool {
    matches!(
        hir.expr(id),
        ExprData::Literal {
            kind: LiteralKind::True,
            ..
        }
    )
}

fn collect_unreachable(hir: &FileHir, statements: IdSlice<StmtId>, out: &mut Vec<TextRange>) {
    Flow {
        never: &|_, _| false,
        dead: Some(out),
    }
    .list(hir, statements);

    // A closure's body hangs off an expression, so no statement walk reaches it -- and dead code in
    // a callback is dead code. Collected first because the walk holds the tree, and recursed into
    // rather than descended, so a closure inside a closure is found by the same rule.
    let mut nested = Vec::new();
    crate::walk::visit_statements(hir, statements, &mut |hir, expr| {
        if let ExprData::Closure { body, .. } = hir.expr(expr) {
            nested.push(*body);
        }
    });
    for body in nested {
        collect_unreachable(hir, hir.body(body).statements, out);
    }
}

/// The span of the statements at the head of `rest` that nothing reaches, or nothing when the first
/// of them is a shape this must not judge.
///
/// Two shapes are stepped around. An [`StmtData::Annotation`] is a docblock rather than a statement,
/// so a `/** @var */` written above dead code is not itself the dead thing and leading ones are
/// skipped. And a statement control can *arrive* at ends the run: PHP hoists a `function` and a
/// `class` declaration whatever they are written after, and a `goto` label is a place control
/// reaches from somewhere else entirely, so neither is dead and neither is anything below it.
///
/// This used to end the run at *every* unmodelled statement, because nothing told a hoisted
/// declaration from an `echo`. [`Passage`] does, and the difference was not the one-line gap it
/// looked like: a dead region *beginning* with an `echo` was dropped whole, and a dead region
/// usually begins with a log line.
fn dead_run(hir: &FileHir, rest: &[StmtId]) -> Option<TextRange> {
    let mut first = None;
    let mut last = None;

    for &id in rest {
        let data = hir.stmt(id);
        match data {
            StmtData::Annotation { .. } if first.is_none() => continue,
            StmtData::Unmodelled {
                passage: Passage::Arrival,
                ..
            } => break,
            _ => {}
        }
        let range = data.range();
        first = first.or(Some(range.start()));
        last = Some(range.end());
    }

    Some(TextRange::new(first?, last?))
}
