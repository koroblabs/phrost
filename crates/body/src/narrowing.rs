//! What an `instanceof` guard or an `assert` call already proved about a value.
//!
//! Only through `&&` and its De Morgan dual through `||`, and only up to the nearest point a branch
//! is guaranteed to leave -- proving more than that would describe code that has not run yet.

use phrost_hir::{
    ExprId, FileHir, StmtId,
    body::{ClassTarget, CompareOp, ExprData, LiteralKind, LogicalOp, NullTestKind, StmtData},
};
use phrost_model::TextSize;

use crate::{
    path::{CallSource, PathRoot, Refused, ValuePath, path_of, same_slot},
    scope::Scope,
    walk::{AssertsArguments, NeverReturns, always_leaves, visit_preceding_statements},
};

/// How many branches deep a claim may be read from.
///
/// One level is the shape that prompted [`Scope::proved_by_both_ways`] -- a fallback inside a
/// guard's arm -- and a second is a fallback inside a fallback, which one real project writes once.
/// Past that the walk costs more than the answer is worth.
pub(crate) const PROVED_DEPTH: u8 = 3;

impl Scope {
    /// The narrowest `instanceof` guard that holds for `subject` at `offset`, and where it was
    /// written.
    ///
    /// Only through `&&`: `$x instanceof A && $y` proves `$x` is an `A` inside the branch, and
    /// `$x instanceof A || $y` proves nothing. Telling those apart is the whole reason the logical
    /// operators are modelled.
    /// What the path that did *not* enter the branch holding `offset` proves about `subject`.
    ///
    /// The mirror of [`Self::narrowing_at`], and deliberately a second reading of the *condition*
    /// rather than the complement of what entering proved. Those are not the same question and the
    /// difference is `||`: entering `if ($best === NULL || $row->min() < $best->min())` proves
    /// nothing, because either disjunct may be the one that held -- while skipping it proves
    /// `$best !== NULL`, because De Morgan says both halves failed. Every find-the-best loop in PHP
    /// is written that way, and complementing the positive answer said `Unproved` for all of them.
    ///
    /// Only the enclosing `if` guards. A guard whose branch *leaves* holds on both paths, so it is
    /// not something a skipped path can be told apart by, and an operand guard is inside the
    /// condition rather than around the branch.
    pub(crate) fn narrowing_skipping(
        &self,
        hir: &FileHir,
        offset: TextSize,
        subject: Subject<'_>,
        asserts: AssertsArguments<'_>,
    ) -> Option<(TextSize, TextSize, Proved)> {
        let mut found = None;
        let facts = crate::facts::BodyFacts::of(hir);
        for guard in facts.if_guards_at(self.body(), offset) {
            let range = hir.expr(guard.condition).range();
            let flags = Flags {
                scope: self,
                at: range.start(),
                budget: FLAG_DEPTH,
                asserts,
            };
            // Innermost last, exactly as `narrowing_at` ranks them: the nearest branch is the one
            // the write sits in, and the one whose complement describes the path around it.
            if let Some(proved) =
                proved_about(hir, guard.condition, subject, !guard.positive, flags)
            {
                found = Some((range.end(), range.start(), proved));
            }
        }

        // And the same for a guard written in the *condition* rather than around the branch.
        // `!$uid && ($uid = $this->uid());` is one statement whose right operand is a write, and
        // the path that skipped it is where `$uid` was already truthy -- which is the whole reason
        // anybody writes the line. Ranked against the enclosing guards by position, as
        // [`Self::narrowing_at`] ranks them.
        for guard in facts.operand_guards_at(self.body(), offset) {
            let range = hir.expr(guard.condition).range();
            let flags = Flags {
                scope: self,
                at: range.start(),
                budget: FLAG_DEPTH,
                asserts,
            };
            if let Some(proved) =
                proved_about(hir, guard.condition, subject, !guard.positive, flags)
                && found
                    .as_ref()
                    .is_none_or(|(previous, _, _)| *previous <= range.end())
            {
                found = Some((range.end(), range.start(), proved));
            }
        }
        found
    }

    /// What `condition` proves about `subject` on the arm `positive` selects.
    pub(crate) fn proved_by_condition(
        &self,
        hir: &FileHir,
        condition: ExprId,
        subject: Subject<'_>,
        positive: bool,
        asserts: AssertsArguments<'_>,
    ) -> Option<Proved> {
        let flags = Flags {
            scope: self,
            at: hir.expr(condition).range().start(),
            budget: FLAG_DEPTH,
            asserts,
        };
        proved_about(hir, condition, subject, positive, flags)
    }

    /// [`proved_about`] for a guard enclosing the position, through the reader's memo if it keeps
    /// one -- see [`crate::proved_memo`].
    fn remembered_guard(
        &self,
        hir: &FileHir,
        condition: ExprId,
        subject: Subject<'_>,
        hash: Option<u64>,
        positive: bool,
        flags: Flags<'_>,
    ) -> Option<Proved> {
        let compute = || proved_about(hir, condition, subject, positive, flags);
        match (flags.asserts.memo(), hash) {
            (Some(memo), Some(hash)) => {
                memo.guard((self.body(), condition, positive), subject, hash, compute)
            }
            _ => compute(),
        }
    }

    /// [`Self::proved_by`] at full depth for a statement before the position, the same way.
    fn remembered_statement(
        &self,
        hir: &FileHir,
        statement: StmtId,
        subject: Subject<'_>,
        hash: Option<u64>,
        never: NeverReturns<'_>,
        asserts: AssertsArguments<'_>,
    ) -> Option<(TextSize, TextSize, Proved)> {
        let compute = || self.proved_by(hir, statement, subject, never, asserts, PROVED_DEPTH);
        // Only an `if`, which is where the cost is -- reading its condition and deciding whether a
        // branch leaves. Anything else answers from its own shape in less than a lookup costs, and
        // remembering those made a Laravel test slower than not remembering at all.
        match (asserts.memo(), hash) {
            (Some(memo), Some(hash)) if matches!(hir.stmt(statement), StmtData::If { .. }) => {
                memo.statement((self.body(), statement), subject, hash, compute)
            }
            _ => compute(),
        }
    }

    pub(crate) fn narrowing_at(
        &self,
        hir: &FileHir,
        offset: TextSize,
        subject: Subject<'_>,
        never: NeverReturns<'_>,
        asserts: AssertsArguments<'_>,
    ) -> Option<(TextSize, ValuePath)> {
        let mut found = None;
        // The subject's hash for the memo, once for every guard and statement below.
        let hash = asserts.memo().map(|_| subject.hash_key());

        // The guards enclosing the position, from the facts derived once for the file rather than a
        // walk of the body per lookup. Outermost first, as the walk emitted them.
        let facts = crate::facts::BodyFacts::of(hir);
        for guard in facts.if_guards_at(self.body(), offset) {
            let condition = guard.condition;
            let range = hir.expr(condition).range();
            let at = range.start();
            let flags = Flags {
                scope: self,
                at,
                budget: FLAG_DEPTH,
                asserts,
            };
            if let Some(proved) =
                self.remembered_guard(hir, condition, subject, hash, guard.positive, flags)
            {
                // Innermost last, so a later guard replaces an outer one: two nested `instanceof`
                // on the same name means the inner is the more specific claim.
                found = Some((range.end(), ValuePath::of(proved.root(at))));
            }
        }

        // A guard in the same expression as the use. Checked before the leaving-guard case below so
        // that the position comparison there ranks the two, rather than order of arrival.
        for guard in facts.operand_guards_at(self.body(), offset) {
            let range = hir.expr(guard.condition).range();
            let at = range.start();
            let flags = Flags {
                scope: self,
                at,
                budget: FLAG_DEPTH,
                asserts,
            };
            if let Some(proved) =
                self.remembered_guard(hir, guard.condition, subject, hash, guard.positive, flags)
                && found
                    .as_ref()
                    .is_none_or(|(previous, _)| *previous <= range.end())
            {
                found = Some((range.end(), ValuePath::of(proved.root(at))));
            }
        }

        // A guard whose branch leaves narrows its *siblings*, not its children: after
        // `if (!($x instanceof T)) { return; }` the value is a `T` for the rest of the list. Later
        // wins, as everywhere else, so this is checked after the enclosing guards.
        visit_preceding_statements(
            hir,
            hir.body(self.body()).statements,
            offset,
            never,
            &mut |hir, statement| {
                if let Some((at, ends, proved)) =
                    self.remembered_statement(hir, statement, subject, hash, never, asserts)
                    && found.as_ref().is_none_or(|(previous, _)| *previous <= ends)
                {
                    found = Some((ends, ValuePath::of(proved.root(at))));
                }
            },
        );

        // A write to the value after the guard makes the guard stale, and the write is what holds.
        // `if (isset($this->application)) { return; } $this->application = $manager->load($id);`
        // is one method in a Drupal form class, and reading the guard there says the property is
        // null for the twenty lines that follow the assignment.
        //
        // Only the guard is dropped, not replaced: what the write put there is the declaration's
        // business, and this layer proves things rather than tracking them.
        //
        // Measured from where the guard *ends*, not where it starts, which is the difference between
        // this working and not on the commonest shape there is:
        // `if (($x = $this->find()) && $x instanceof Special)` writes to the subject inside its own
        // condition, so a staleness window opening at the condition's start declared the guard stale
        // before it was finished being read -- and dropped it. The same offset is what ranks a guard
        // against an assignment, and for the same reason: a guard written *around* an assignment is
        // about the value that assignment produced.
        let (ends, _) = found.as_ref()?;
        match self.assigned_between(hir, subject, *ends, offset, never) {
            true => None,
            false => found,
        }
    }

    /// Whether the subject is written to strictly between two positions.
    ///
    /// Only by a write control had to go through. A write in a branch the position is not on never
    /// happened on the way here, so it cannot have made the guard stale --
    /// `if ($e === NULL) { $e = make(); } else { $e->run(); }` proves non-null in the else, and
    /// counting the then-branch's write here dropped that proof and brought the null back.
    fn assigned_between(
        &self,
        hir: &FileHir,
        subject: Subject<'_>,
        after: TextSize,
        before: TextSize,
        never: NeverReturns<'_>,
    ) -> bool {
        let mut written = false;

        let facts = crate::facts::BodyFacts::of(hir);
        for site in facts.writes_that_may_root_at(self.body(), subject.written_root()) {
            if !subject.may_be_written_by(site) {
                continue;
            }
            let range = site.range;
            if !(range.start() > after && range.end() <= before) {
                continue;
            }
            if facts.reach_of(hir, site, before, never) != Some(crate::reach::Reach::Certain) {
                continue;
            }
            let ExprData::Assign { target, .. } = hir.expr(site.expr) else {
                continue;
            };
            if subject.is(hir, *target) {
                written = true;
            }
        }

        written
    }

    /// What a statement standing before the cursor proves about `subject`.
    ///
    /// Three shapes, all of which say "past this point, the value is a `T`": a guard whose branch
    /// cannot fall through, an `assert`, and an `if` both of whose ways out agree.
    ///
    /// `depth` bounds the last of those, which reads the statements of a branch and so reaches this
    /// again one level in.
    pub(crate) fn proved_by(
        &self,
        hir: &FileHir,
        statement: StmtId,
        subject: Subject<'_>,
        never: NeverReturns<'_>,
        asserts: AssertsArguments<'_>,
        depth: u8,
    ) -> Option<(TextSize, TextSize, Proved)> {
        let range = hir.stmt(statement).range();

        let flags = Flags {
            scope: self,
            at: range.start(),
            budget: FLAG_DEPTH,
            asserts,
        };
        if let Some(proved) = asserted_by_call(hir, statement, subject, flags) {
            return Some((range.start(), range.end(), proved));
        }

        let StmtData::If {
            condition,
            then_branch,
            else_branch,
            ..
        } = hir.stmt(statement)
        else {
            return None;
        };

        // `if (!(cond)) { return; }` proves what `cond` does; `if (cond) { ... } else { return; }`
        // proves it as well, from the other side.
        let flags = Flags {
            scope: self,
            at: hir.expr(*condition).range().start(),
            budget: FLAG_DEPTH,
            asserts,
        };
        // Resolved from the statement's start and ranked from its end: what a leaving guard proves
        // holds for its *siblings*, so anything written after the whole `if` is later than it.
        // What the condition proves is asked first, and the order is a measurement rather than a
        // preference: deciding whether a branch leaves may have to resolve a call -- `fail()` is
        // only known to end things by its declaration -- while reading the condition is local. Most
        // guards in a function say nothing about the value being resolved, and this way those cost
        // what they always did.
        if let Some(proved) = proved_about(hir, *condition, subject, false, flags)
            && always_leaves(hir, *then_branch, never)
        {
            // An `elseif` whose branch also leaves proves its own negation too, and the code after
            // the whole chain is past both of them: `if (A) return; elseif (B) die();` leaves
            // neither an A nor a B. Composed rather than ranked, for the reason `and_also` gives --
            // taking whichever claim was stronger would rule out one and forget the other.
            let proved = match self.chained_guard(hir, *else_branch, subject, never, asserts) {
                Some(more) => more.about(ValuePath::of(proved.root(range.start()))),
                None => proved,
            };
            return Some((range.start(), range.end(), proved));
        }
        if let Some(proved) = proved_about(hir, *condition, subject, true, flags)
            && always_leaves(hir, *else_branch, never)
        {
            return Some((range.start(), range.end(), proved));
        }

        // The shape whose guard does not mention the value at all.
        if let Some(proved) =
            self.proved_by_an_empty_collector(hir, statement, subject, never, asserts)
        {
            return Some((range.start(), range.end(), proved));
        }

        // And the shape where the arm comes back out, and what it did before coming out is what
        // the condition's complement says.
        self.proved_by_both_ways(hir, statement, subject, never, asserts, depth)
            .map(|proved| (range.start(), range.end(), proved))
    }

    /// What an `if` whose arm comes back out proves for the code after it.
    ///
    /// The arm above answers for an arm that *leaves*: after `if (!($app = $this->load($id))) {
    /// return; }` the value is what the condition ruled in. An arm that comes back out proves
    /// nothing on its own -- unless what it does before coming out establishes the same thing the
    /// condition's complement does, which is exactly how a fallback is written:
    ///
    /// ```php
    /// if (!($app = $this->manager->loadActive($id))) {
    ///     if (!($app = $this->manager->loadByUserId($id))) {
    ///         return $this->error();
    ///     }
    /// }
    /// $app->isBulkOrder();
    /// ```
    ///
    /// Both ways out leave `$app` non-null: not taking the outer arm means the first load answered,
    /// and coming back out of it means the second did. Read as two unrelated guards the last line
    /// is a `possibly-null-receiver`, and the inner guard is invisible from below --
    /// `visit_preceding_statements` does not descend into a *completed* branch, which is right in
    /// general and is exactly what this arm supplies for the one case where it is not.
    ///
    /// **Only about null.** The vocabulary has several kinds of claim and "the same claim on both
    /// paths" would need a lattice over them that nothing else here wants; a null is the one a
    /// fallback is written about, and the one that reads as a false positive when it survives.
    fn proved_by_both_ways(
        &self,
        hir: &FileHir,
        statement: StmtId,
        subject: Subject<'_>,
        never: NeverReturns<'_>,
        asserts: AssertsArguments<'_>,
        depth: u8,
    ) -> Option<Proved> {
        let depth = depth.checked_sub(1)?;
        let StmtData::If {
            condition,
            then_branch,
            else_branch,
            ..
        } = hir.stmt(statement)
        else {
            return None;
        };
        // An arm that always leaves is the arm above, and it has already answered.
        if always_leaves(hir, *then_branch, never) {
            return None;
        }

        let taken = self.claim_through(hir, *then_branch, subject, never, asserts, depth)?;
        let skipped = match hir.stmt_list(*else_branch).is_empty() {
            // No `else`, so the path that skipped the arm is the one the condition rules out.
            true => {
                let flags = Flags {
                    scope: self,
                    at: hir.expr(*condition).range().start(),
                    budget: FLAG_DEPTH,
                    asserts,
                };
                proved_about(hir, *condition, subject, false, flags)?
            }
            false => self.claim_through(hir, *else_branch, subject, never, asserts, depth)?,
        };

        match (taken, skipped) {
            (
                Proved::NotNull { of, refused: taken },
                Proved::NotNull {
                    refused: skipped, ..
                },
            ) => Some(Proved::NotNull {
                of,
                // The weaker of the two claims, which is the one both paths make.
                refused: taken.weaker_of(skipped),
            }),
            _ => None,
        }
    }

    /// What still holds about `subject` at the end of a branch, on the paths that reach it.
    ///
    /// The last thing the branch's own statements prove, which is [`Self::proved_by`] one level in
    /// -- and nothing at all where a write to the subject stands after it. A write is refused
    /// whether or not control had to go through it: a claim and a write nobody can choose between
    /// is the shape `slot_value` withdraws for, and here the claim is one operand of an agreement.
    pub(crate) fn claim_through(
        &self,
        hir: &FileHir,
        branch: lspkit_arena::IdSlice<StmtId>,
        subject: Subject<'_>,
        never: NeverReturns<'_>,
        asserts: AssertsArguments<'_>,
        depth: u8,
    ) -> Option<Proved> {
        let mut found: Option<Proved> = None;
        for &statement in hir.stmt_list(branch) {
            // Before the claim, not after: the write a guard's own condition makes -- the `$app =`
            // inside `if (!($app = ...))` -- is what the guard is about.
            if self.writes_subject(hir, statement, subject) {
                found = None;
            }
            if let Some((_, _, proved)) =
                self.proved_by(hir, statement, subject, never, asserts, depth)
            {
                found = Some(proved);
            }
        }
        found
    }

    /// Whether anything inside `statement` assigns to the subject.
    fn writes_subject(&self, hir: &FileHir, statement: StmtId, subject: Subject<'_>) -> bool {
        let range = hir.stmt(statement).range();
        let facts = crate::facts::BodyFacts::of(hir);
        facts.writes_within(self.body(), range).any(|site| {
            subject.may_be_written_by(site)
                && matches!(hir.expr(site.expr), ExprData::Assign { target, .. } if subject.is(hir, *target))
        })
    }

    /// What a collector nobody filled proves about the values that would have filled it.
    ///
    /// ```php
    /// $errors = [];
    /// if ($shipping === NULL) {
    ///     $errors[] = $this->dateError('shipping');
    /// }
    /// if ($errors !== []) {
    ///     return $this->validationError($errors);
    /// }
    /// $order->setDueDate($shipping);
    /// ```
    ///
    /// Collecting the reasons a request is invalid and bailing out on the list is how application
    /// code validates -- it is the thing a framework's constraint objects exist to replace, which is
    /// why the corpora barely write it and one method of the reference project writes three of them.
    /// Read as three unrelated guards, the last line reports a null that cannot arrive: the `if`
    /// about `$shipping` is behind the cursor and a branch that may not have run proves nothing to
    /// its siblings, and the guard that does leave is about `$errors`.
    ///
    /// The step between them is exact rather than a heuristic. An append always leaves the collector
    /// holding something, so a collector that is *empty* here is one no append ran on, so every
    /// condition that would have appended is false -- and `!($shipping === NULL)` is then an ordinary
    /// claim, put to [`proved_about`] exactly as the negated-guard arm above puts its own.
    ///
    /// What makes it sound is what it refuses.
    ///
    /// Every mention of the collector before the guard has to *be* one of the appends. `array_pop`,
    /// `array_filter`, an `unset`, a by-reference argument, a `use (&$errors)` capture, even a plain
    /// read: each means the emptiness may have another cause, and counting mentions against appends
    /// is how they are told apart with no dataflow model to ask.
    ///
    /// The append has to be a direct statement of the arm for the arm's condition to be the claim.
    /// Nested one level in, what an empty collector rules out is the *pair* of conditions and this
    /// would be claiming the outer one -- so such an append is read as accounted for and proves
    /// nothing, which loses a claim and invalidates none. An `elseif` chain falls out of the same
    /// rule rather than needing its own: the first arm's condition is claimed because reaching an
    /// empty collector means that arm did not run, and a later arm's is not, because ruling it out
    /// would need every arm above it ruled out first.
    ///
    /// A write to the subject *between* the check and the guard drops that check's claim. What an
    /// empty collector rules out is the value the check looked at, and anything written since is a
    /// different value -- the staleness [`Scope::narrowing_at`] measures after a guard, measured
    /// before this one.
    ///
    /// An assignment of the whole collector voids every claim above it and none below it: what it
    /// was given may be non-empty, and if it was then the guard leaves and none of this code runs.
    ///
    /// The statements read are the body's own, so the guard and the appends have to be siblings
    /// there -- a guard nested in a branch declines, which is the miss this deliberately keeps
    /// rather than reasoning about which list a `StmtId` sits in.
    fn proved_by_an_empty_collector(
        &self,
        hir: &FileHir,
        statement: StmtId,
        subject: Subject<'_>,
        never: NeverReturns<'_>,
        asserts: AssertsArguments<'_>,
    ) -> Option<Proved> {
        let StmtData::If {
            condition,
            then_branch,
            ..
        } = hir.stmt(statement)
        else {
            return None;
        };
        let collector = a_non_empty_test(hir, *condition)?;
        if !always_leaves(hir, *then_branch, never) {
            return None;
        }

        let statements = hir.stmt_list(hir.body(self.body()).statements);
        let guard = statements.iter().position(|held| *held == statement)?;
        let flags = Flags {
            scope: self,
            at: hir.stmt(statement).range().start(),
            budget: FLAG_DEPTH,
            asserts,
        };

        let mut proved: Option<Proved> = None;
        for &earlier in &statements[..guard] {
            match collecting(hir, earlier, collector) {
                Collecting::Elsewhere | Collecting::Elsewhen => {}
                Collecting::Replaced => proved = None,
                Collecting::Unaccounted => return None,
                Collecting::Under(condition) => {
                    // A write to the subject between the check and the guard makes the complement
                    // stale: what an empty collector rules out is what the *check* saw, and the
                    // value here is whatever was written since. The same staleness `narrowing_at`
                    // measures on the other side of the guard, measured on this side of it.
                    if self.assigned_between(
                        hir,
                        subject,
                        hir.stmt(earlier).range().end(),
                        flags.at,
                        never,
                    ) {
                        continue;
                    }
                    let Some(more) = proved_about(hir, condition, subject, false, flags) else {
                        continue;
                    };
                    // Composed rather than ranked, for the reason `and_also` gives: two appends can
                    // be guarded by two conditions that each say something about one value.
                    proved = Some(match proved {
                        Some(held) => held.and_also(more, flags.at),
                        None => more,
                    });
                }
            }
        }

        proved
    }

    /// What the rest of an `if` chain proves, when the `elseif` branches leave as well.
    ///
    /// An `elseif` is an `if` inside the else, so this is the same reader applied one level in. Only
    /// a lone `if` there is a chain: an `else` with statements beside it is an ordinary else, and the
    /// branch above already answered for it.
    fn chained_guard(
        &self,
        hir: &FileHir,
        else_branch: lspkit_arena::IdSlice<StmtId>,
        subject: Subject<'_>,
        never: NeverReturns<'_>,
        asserts: AssertsArguments<'_>,
    ) -> Option<Proved> {
        let [only] = hir.stmt_list(else_branch) else {
            return None;
        };
        if !matches!(hir.stmt(*only), StmtData::If { .. }) {
            return None;
        }
        self.proved_by(hir, *only, subject, never, asserts, PROVED_DEPTH)
            .map(|(_, _, proved)| proved)
    }
}

/// How a statement uses a collector, which is what decides whether its emptiness proves anything.
enum Collecting {
    /// Not mentioned in it.
    Elsewhere,
    /// Appended to by a direct statement of the arm of an `if` whose condition this is. An empty
    /// collector therefore rules that condition out.
    Under(ExprId),
    /// Appended to somewhere this reader cannot attribute to one condition -- inside a nested `if`,
    /// inside a loop, or unconditionally. An empty collector rules *something* out and nothing here
    /// can say what, which loses a claim and invalidates none: what the other appends prove is
    /// still that they did not run.
    Elsewhen,
    /// Assigned as a whole, which voids every claim made above it and none below. Whatever it was
    /// given may be non-empty, and an empty collector at the guard says only that it is empty
    /// *there* -- so an append after this one still proves its condition false.
    Replaced,
    /// Mentioned in a way this reader cannot account for -- read, taken from, passed on.
    Unaccounted,
}

/// Which of the five `statement` is, for the local called `collector`.
fn collecting(hir: &FileHir, statement: StmtId, collector: &str) -> Collecting {
    // Every occurrence in the statement, nested statements included, against the ones that are an
    // append's own target. Equal means each occurrence *is* an append: the base variable of one is
    // the only mention it makes, so anything else the statement does with the collector shows up
    // here as a mention with no append to pair with.
    let mentions = mentions_of(hir, statement, collector);
    if mentions == 0 {
        return Collecting::Elsewhere;
    }
    if appends_anywhere(hir, statement, collector) != mentions {
        return match hir.stmt(statement) {
            StmtData::Expr { expr, .. } if mentions == 1 && replaces(hir, *expr, collector) => {
                Collecting::Replaced
            }
            _ => Collecting::Unaccounted,
        };
    }

    // One direct append in the arm is enough to make the condition the whole reason: if it held,
    // that statement ran, and the collector is not empty. Others nested deeper inside the same arm
    // are then beside the point rather than a problem.
    match hir.stmt(statement) {
        StmtData::If {
            condition,
            then_branch,
            ..
        } if appended_directly(hir, *then_branch, collector) > 0 => Collecting::Under(*condition),
        _ => Collecting::Elsewhen,
    }
}

/// How many times anything in `statement` appends to `collector`.
fn appends_anywhere(hir: &FileHir, statement: StmtId, collector: &str) -> usize {
    let mut count = 0;
    crate::walk::visit_one_statement(hir, statement, &mut |hir, expr| {
        if let ExprData::Assign { target, .. } = hir.expr(expr)
            && appends_to(hir, *target, collector)
        {
            count += 1;
        }
    });
    count
}

/// Whether an assignment target is `$collector[]`.
fn appends_to(hir: &FileHir, target: ExprId, collector: &str) -> bool {
    matches!(
        hir.expr(target),
        ExprData::Index { base, index: None, .. } if local_named(hir, *base) == Some(collector)
    )
}

/// How many times `collector` is named anywhere in `statement`.
fn mentions_of(hir: &FileHir, statement: StmtId, collector: &str) -> usize {
    let mut count = 0;
    crate::walk::visit_one_statement(hir, statement, &mut |hir, expr| {
        if let ExprData::Variable { name, .. } = hir.expr(expr)
            && name.as_str() == collector
        {
            count += 1;
        }
    });
    count
}

/// How many direct statements of `branch` append to `collector`.
///
/// Direct, because a nested one is guarded by something this reader is not looking at: what an empty
/// collector then rules out is the pair of conditions and not the arm's own.
fn appended_directly(
    hir: &FileHir,
    branch: lspkit_arena::IdSlice<StmtId>,
    collector: &str,
) -> usize {
    hir.stmt_list(branch)
        .iter()
        .filter(|&&statement| {
            let StmtData::Expr { expr, .. } = hir.stmt(statement) else {
                return false;
            };
            let ExprData::Assign { target, .. } = hir.expr(*expr) else {
                return false;
            };
            appends_to(hir, *target, collector)
        })
        .count()
}

/// Whether the expression assigns the whole of `collector`.
///
/// What is assigned does not matter, and that is the point: if it left the collector holding
/// something the guard leaves and nothing below it runs, so an append written after this one still
/// proves its own condition false. Only the claims made *above* it are lost.
fn replaces(hir: &FileHir, expr: ExprId, collector: &str) -> bool {
    let ExprData::Assign { target, .. } = hir.expr(expr) else {
        return false;
    };
    local_named(hir, *target) == Some(collector)
}

/// The name of the local a condition tests for holding anything, where it tests one.
///
/// The four spellings of "this collector has something in it". `empty`, `count` and truthiness are
/// falsehood tests rather than emptiness tests, which for an array is the same question -- and it is
/// an array, because the rule this serves needs an append on it.
///
/// `count($errors) > 0` is **not** here, and the reason is worth stating: `ExprData::Literal` keeps
/// a kind and a range and not a value, so nothing at this layer can tell `> 0` from `> 3` -- and
/// reading `> 3` as an emptiness test would claim the appends did not run where three of them did.
/// A miss rather than a guess.
fn a_non_empty_test(hir: &FileHir, condition: ExprId) -> Option<&str> {
    match hir.expr(condition) {
        ExprData::Comparison {
            operator,
            left,
            right,
            ..
        } => holds_something(hir, *operator, *left, *right),
        // `!empty($errors)`. The negation is the `Not` around it, so this arm is the affirmative
        // spelling of the test and the emptiness is what the *other* branch has.
        ExprData::Not { value, .. } => match hir.expr(*value) {
            ExprData::NullTest {
                value,
                kind: NullTestKind::Empty,
                ..
            } => local_named(hir, *value),
            _ => None,
        },
        ExprData::Variable { name, .. } => Some(name.as_str()),
        // `if (count($errors))`, which is the same falsehood test written through a call.
        ExprData::FunctionCall { .. } => counted(hir, condition),
        _ => None,
    }
}

/// `$errors !== []`, and the way round it can be written.
fn holds_something(
    hir: &FileHir,
    operator: CompareOp,
    left: ExprId,
    right: ExprId,
) -> Option<&str> {
    // `!==` and `!=` are symmetric, so both orders are the one test -- and the reversed order is how
    // a codebase that writes Yoda comparisons writes every one of them.
    if !matches!(operator, CompareOp::NotIdentical | CompareOp::NotEqual) {
        return None;
    }
    [(left, right), (right, left)]
        .into_iter()
        .find_map(|(value, against)| {
            is_an_empty_array(hir, against)
                .then(|| local_named(hir, value))
                .flatten()
        })
}

/// The local `count($errors)` counts, where the expression is that call.
fn counted(hir: &FileHir, expr: ExprId) -> Option<&str> {
    let ExprData::FunctionCall { name, args, .. } = hir.expr(expr) else {
        return None;
    };
    let called = hir.name_ref(*name).path.as_str();
    if !called.eq_ignore_ascii_case("count") && !called.eq_ignore_ascii_case("sizeof") {
        return None;
    }
    let [only] = hir.expr_list(*args) else {
        return None;
    };
    local_named(hir, argument_value(hir, *only))
}

/// The value an argument passes, which a named argument wraps.
fn argument_value(hir: &FileHir, expr: ExprId) -> ExprId {
    match hir.expr(expr) {
        ExprData::Argument { value, .. } => *value,
        _ => expr,
    }
}

/// The name of a plain local, where the expression is one.
fn local_named(hir: &FileHir, expr: ExprId) -> Option<&str> {
    match hir.expr(expr) {
        ExprData::Variable { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

/// Whether the expression is the literal `[]`.
fn is_an_empty_array(hir: &FileHir, expr: ExprId) -> bool {
    matches!(hir.expr(expr), ExprData::ArrayLiteral { elements, .. } if hir.expr_list(*elements).is_empty())
}

/// The class a guard proves for the expression `subject` names, at `offset`.
///
/// The counterpart of the narrowing that [`local_value`](crate::local_value) already folds in for a
/// plain local, for every other expression a guard can be written about. It exists because a guard
/// is sometimes the only claim in the file: `protected array $users = []` says nothing about what
/// is in it, and
///
/// ```php
/// if ($this->users[$uid] instanceof User) {
///     return $this->users[$uid]->hasPermission($permission);
/// }
/// ```
///
/// is how a keyed cache of entities is always written. Nothing else there says `User`.
///
/// Deliberately *not* used for a bare local: [`local_value`](crate::local_value) already ranks a
/// narrowing against the assignments around it, and a guard that always won would undo a later
/// assignment.
pub fn narrowed_at(
    hir: &FileHir,
    offset: TextSize,
    subject: &ValuePath,
    never: NeverReturns<'_>,
    asserts: AssertsArguments<'_>,
) -> Option<ValuePath> {
    narrowed_with_position(hir, offset, subject, never, asserts).map(|(_, path)| path)
}

/// The same, keeping where the guard was written.
///
/// For [`crate::slot_value`], which has a second claim about the same value to rank against it and
/// cannot tell which is the later one without this.
pub(crate) fn narrowed_with_position(
    hir: &FileHir,
    offset: TextSize,
    subject: &ValuePath,
    never: NeverReturns<'_>,
    asserts: AssertsArguments<'_>,
) -> Option<(TextSize, ValuePath)> {
    for scope in Scope::chain_at(hir, offset).iter().rev() {
        if let Some(found) =
            scope.narrowing_at(hir, offset, Subject::Expression(subject), never, asserts)
        {
            return Some(found);
        }
    }
    None
}

/// The class a guard proves `$this` is, at `offset`.
///
/// Its own entry rather than a `ValuePath` the caller builds, because `$this` is the one receiver
/// whose path nobody writes down: it is not a local, so [`local_value`](crate::local_value) never
/// sees it, and until this existed it was always the class the method was declared in.
///
/// Which is wrong in the shape PHP uses instead of pattern matching. A base class asking what it
/// itself is --
///
/// ```php
/// if ($this instanceof RefurbishedKitApplication) {
///     $exclude = $this->getRefundInfo()['amount'] ?? 0;
/// }
/// ```
///
/// -- is a method that exists on the subclass and not on the class the code is written in, so
/// without the guard the call reads as undefined and the diagnostic says so.
pub fn narrowed_this(
    hir: &FileHir,
    offset: TextSize,
    never: NeverReturns<'_>,
    asserts: AssertsArguments<'_>,
) -> Option<ValuePath> {
    narrowed_at(hir, offset, &ValuePath::of(PathRoot::This), never, asserts)
}

/// What was called, as the file spells it.
///
/// Two shapes and not one, because the reader looks them up in different tables: a method is found
/// on a class, and a function by the name it was written with.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Callee {
    Member {
        receiver: ValuePath,
        name: phrost_model::Name,
    },
    Function(phrost_hir::NameRefId),
    /// `new Repo($out)` -- the constructor's parameters are the ones to read.
    Constructed(ClassTarget),
}

/// The last call before `offset` that passed `$name` as an argument.
///
/// For a name nothing else binds. `foo($bar)` is how PHP writes an out-parameter, and the variable
/// exists only because the call wrote to it -- so there is no assignment, no parameter and no loop
/// for the usual search to find.
///
/// The last one, because a later call overwrites what an earlier one put there.
pub fn passed_by_reference(hir: &FileHir, offset: TextSize, name: &str) -> Option<ValuePath> {
    let mut found: Option<(TextSize, ValuePath)> = None;

    for scope in Scope::chain_at(hir, offset).iter() {
        // Every call that ran, not only the ones written as a statement of their own. Nobody writes
        // `preg_match($p, $s, $m);` on its own line -- they write `if (preg_match($p, $s, $m))` --
        // and gating on the statement shape meant the name was not bound at all in three of the four
        // ways the function is written. See [`visit_calls_before`].
        crate::walk::visit_calls_before(
            hir,
            hir.body(scope.body()).statements,
            offset,
            &mut |hir, expr, _| {
                let Some((callee, args)) = callee_of(hir, expr) else {
                    return;
                };
                let at = hir.expr(expr).range().start();
                let _ = &callee;
                for &passed in hir.expr_list(args).iter() {
                    let passed = match hir.expr(passed) {
                        ExprData::Argument { value, .. } => *value,
                        _ => passed,
                    };
                    if !Subject::Local(name).is(hir, passed) {
                        continue;
                    }
                    // The newest position, and nothing about *which* call: the reader walks them.
                    let Some(subject) = path_of(hir, passed) else {
                        continue;
                    };
                    if found.as_ref().is_none_or(|(seen, _)| *seen <= at) {
                        found = Some((
                            at,
                            ValuePath::of(PathRoot::ByReference {
                                subject: Box::new(subject),
                                at,
                            }),
                        ));
                    }
                }
            },
        );
    }

    found.map(|(_, path)| path)
}

/// What a call statement calls, and the arguments it was given.
fn callee_of(hir: &FileHir, expr: ExprId) -> Option<(Callee, lspkit_arena::IdSlice<ExprId>)> {
    match hir.expr(expr) {
        ExprData::MethodCall {
            receiver,
            name: Some(method),
            args,
            ..
        } => Some((
            Callee::Member {
                receiver: path_of(hir, *receiver)?,
                name: method.clone(),
            },
            *args,
        )),
        ExprData::StaticCall {
            class,
            name: Some(method),
            args,
            ..
        } => Some((
            Callee::Member {
                receiver: ValuePath::of(crate::path::class_path_root(*class)),
                name: method.clone(),
            },
            *args,
        )),
        ExprData::FunctionCall { name, args, .. } => Some((Callee::Function(*name), *args)),
        ExprData::New { class, args, range } => {
            let _ = range;
            Some((Callee::Constructed(*class), *args))
        }
        _ => None,
    }
}

/// A call written as a statement, which may have done something to what it was passed.
///
/// May, because what it did is written on the *callee* -- `@phpstan-assert !null $param`, a
/// by-reference parameter, or a `@phpstan-self-out` about the value it was called on -- in whatever
/// file that is, and this layer reads one. So it names the call and which part of it the value
/// was, and the reader looks the declaration up.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PassedTo {
    /// Where the call was written. The receiver is resolved there, and so before the call.
    pub at: TextSize,
    pub callee: Callee,
    /// Which part of the call the value was.
    pub passed_as: PassedAs,
    /// The parameter it was written under, for `f(bar: $out)`.
    ///
    /// Which parameter a name fills takes the callee's signature to say, which this layer has none
    /// of -- so the name travels and the reader matches it. `None` for a positional argument.
    pub name: Option<phrost_model::Name>,
    /// The class each argument names, for the arguments written `X::class`.
    ///
    /// Carried because some contracts say only *which* argument names what they prove --
    /// `assertInstanceOf(Foo::class, $actual)` puts the class at the call and the declaration only
    /// says where to look. One entry per argument, so a position lines up with what the tag names.
    pub class_arguments: Vec<Option<ClassTarget>>,
}

/// Which part of a call a value was handed over as.
///
/// An enum rather than a position with a flag beside it, so a reader that matches an argument to a
/// parameter has to say what it does with a receiver -- read as position zero, `$box->replace($x)`
/// would apply the first parameter's `@param-out` to `$box`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PassedAs {
    /// An argument, counted from zero.
    Argument(usize),
    /// The value the method was called on: `$box->replace($next)` hands `$box` over as surely as
    /// it hands `$next`, and the declaration may say what `$box` holds once it returns. The call
    /// travels with it, because that declaration's arguments are filled from the call's.
    Receiver(CallSource),
}

/// Every call before `offset` that passed `subject` as an argument.
///
/// Every call, and each entry says whether it was a statement of its own -- see
/// [`PassedTo::at_statement`], which is where the two readers of this list part company. Kept
/// separate from the guards, which replace what a value is; an assertion only rules the null out, so
/// the reader applies it to whatever the declaration said rather than instead of it -- and `$x` is
/// still "assigned" in hover afterwards, which it is.
///
/// Asked only about a value that could actually be null, because the walk is not free. See
/// [`IndexView::resolve_path`](../../phrost_view/index.html).
/// Every call that *ran* before `offset` and passed `subject`, whatever position it was written in.
///
/// The wide sibling of [`passed_to_calls`], and the two are separate because they answer different
/// questions and cost different amounts.
///
/// That one is asked about every value whose declaration was vague, which on a real project is most
/// of them, so it stays narrow: only a call written as a statement of its own, which is the shape an
/// `@assert` tag is written on. Widening *it* cost `phrost check` 2.42 s -> 3.15 s on the reference
/// project, +30 %, and the walk is the same walk.
///
/// This one is asked only when nothing else bound the name at all -- the [`PathRoot::ByReference`]
/// reader -- which is rare, and it is where the width is needed. A by-reference parameter writes to
/// the caller's variable wherever the call was written, and eleven shapes of one function are not
/// eleven statements:
///
/// ```php
/// preg_match($p, $s, $m);              // the only one the narrow walk sees
/// if (preg_match($p, $s, $m)) { }      // what everybody writes
/// if (!preg_match($p, $s, $m)) { }
/// if (preg_match($p, $s, $m)) {} else { }
/// $ok = preg_match($p, $s, $m);
/// while (preg_match($p, $s, $m)) { }
/// ```
///
/// # The one unsound shape, kept deliberately
///
/// `if ($flag && preg_match($p, $s, $m)) {} else { $m }` claims a type where the call may not have
/// run. Dropping calls under a short-circuit would also lose
/// `if ($flag && preg_match($p, $s, $m)) { $m }`, which is sound and is written far more often -- so
/// the trade is one wrong answer in the `else` of a short-circuited condition against every right one
/// in the branch it guards. Answering the `else` correctly is a question about which branch a value
/// reached, which is [`crate::reach`]'s and not this walk's.
pub fn passed_to_any_call(hir: &FileHir, offset: TextSize, subject: &ValuePath) -> Vec<PassedTo> {
    let mut checks = Vec::new();
    for scope in Scope::chain_at(hir, offset).iter() {
        crate::walk::visit_calls_before(
            hir,
            hir.body(scope.body()).statements,
            offset,
            // No receiver: what this walk serves is a by-reference parameter, which a receiver
            // never fills.
            &mut |hir, expr, _| {
                collect_passed(hir, expr, subject, &|_| false, &mut checks);
            },
        );
    }
    checks
}

///
/// `among` is the statements [`PassedPaths::statements`] said may pass `subject`, sorted: the walk
/// still decides which of them precede the position, and only they are read. `leaves_receiver` is
/// the name filter the index was built with, and says which calls made *on* `subject` count.
///
/// `speaks` is asked of a statement that precedes the position, before anything else is read off
/// it: whether its callee has anything to say about the value at all. Only of those, and that
/// is the point of asking it here rather than of every statement the index holds -- answering it
/// resolves the callee, which for a method resolves its receiver *where the call was written*, so
/// asked of a statement at or after the position it comes back to this walk for the same value
/// and stops only when the follow budget does.
pub fn passed_to_calls(
    hir: &FileHir,
    offset: TextSize,
    subject: &ValuePath,
    among: &[StmtId],
    leaves_receiver: &dyn Fn(&str) -> bool,
    speaks: &dyn Fn(StmtId) -> bool,
) -> Vec<PassedTo> {
    let mut checks = Vec::new();
    if among.is_empty() {
        return checks;
    }

    for scope in Scope::chain_at(hir, offset).iter() {
        visit_preceding_statements(
            hir,
            hir.body(scope.body()).statements,
            offset,
            // Nothing here can look a callee up, so a catch whose last statement is a call to
            // something declared `never` does not count as leaving. Conservative in the direction
            // that descends less, which is the safe one: the walk then misses an assertion rather
            // than believing one that may not have run.
            &|_, _| false,
            &mut |hir, statement| {
                if among.binary_search(&statement).is_err() || !speaks(statement) {
                    return;
                }
                let StmtData::Expr { expr, .. } = hir.stmt(statement) else {
                    return;
                };
                // A write past the call makes what the call said about the value stale, which is
                // the rule a guard already answers to in `narrowing_at` -- and a call statement is
                // read here as exactly that, a guard. `sort($term); $term = implode(',', $term);`
                // left the by-reference `array` standing over the string written after it, and
                // every check downstream read the name as the array it no longer was.
                if scope.assigned_between(
                    hir,
                    Subject::Expression(subject),
                    hir.stmt(statement).range().end(),
                    offset,
                    &|_, _| false,
                ) {
                    return;
                }
                collect_passed(hir, *expr, subject, leaves_receiver, &mut checks);
            },
        );
    }

    checks
}

/// What one statement's call was handed `subject` as: the entries [`passed_to_calls`] collects from
/// it, without asking whether it precedes anything.
///
/// For a reader that settles something about a statement once -- whether its callee says anything
/// about the value at all -- rather than once per position that reads the value.
pub fn passed_by_statement(
    hir: &FileHir,
    statement: StmtId,
    subject: &ValuePath,
    leaves_receiver: &dyn Fn(&str) -> bool,
) -> Vec<PassedTo> {
    let mut checks = Vec::new();
    if let StmtData::Expr { expr, .. } = hir.stmt(statement) {
        collect_passed(hir, *expr, subject, leaves_receiver, &mut checks);
    }
    checks
}

/// Which statement calls hand each path over, for the whole file -- the index behind
/// [`passed_to_calls`].
///
/// That walk is asked per position: every time a value is resolved, the statements before it are
/// visited for a call the value was passed to, and each call visited had its receiver and arguments
/// read. A receiver written two hundred times in one function -- `$scripts->add(...)` in
/// WordPress's `script-loader.php` -- paid for that two hundred times over, and the file took seven
/// seconds to check. Which statements pass which path is a property of the tree, settled once; the
/// walk keeps deciding which of them *precede* the position, and reads only those.
///
/// A superset by construction: every call shape the walk reads is read here, in every scope, and
/// each path is kept without its nullsafe flag, which is the one field
/// [`same_slot`](crate::path::same_slot) does not compare. The one thing left out is a value with
/// no name -- a literal, a cast -- which no call can have said anything lasting about; see
/// [`ValuePath::names_a_value`].
///
/// A method call's receiver is recorded only where `leaves_receiver` says its name may be declared
/// to change what the receiver holds. Every receiver of every call is most of the file, and a path
/// with statements recorded is one the walk then runs for -- so the name filter is what keeps a
/// value that is only ever called on out of the walk entirely.
#[derive(Debug, Default)]
pub struct PassedPaths(std::collections::HashMap<ValuePath, Vec<StmtId>>);

impl PassedPaths {
    pub fn of(hir: &FileHir, leaves_receiver: &dyn Fn(&str) -> bool) -> Self {
        let mut passed: std::collections::HashMap<ValuePath, Vec<StmtId>> =
            std::collections::HashMap::new();
        for scope in Scope::every_scope(hir) {
            crate::walk::visit_every_statement(
                hir,
                hir.body(scope.body()).statements,
                &mut |hir, statement| {
                    let StmtData::Expr { expr, .. } = hir.stmt(statement) else {
                        return;
                    };
                    if let Some(mut path) = receiver_left_changed(hir, *expr, leaves_receiver) {
                        path.may_be_absent = false;
                        let statements = passed.entry(path).or_default();
                        if statements.last() != Some(&statement) {
                            statements.push(statement);
                        }
                    }
                    let args = match hir.expr(*expr) {
                        ExprData::MethodCall { args, .. }
                        | ExprData::StaticCall { args, .. }
                        | ExprData::FunctionCall { args, .. }
                        | ExprData::New { args, .. } => args,
                        _ => return,
                    };
                    for &argument in hir.expr_list(*args) {
                        let value = match hir.expr(argument) {
                            ExprData::Argument { value, .. } => *value,
                            _ => argument,
                        };
                        // The target of an assignment passed in place, as `collect_passed` reads it.
                        let value = match hir.expr(value) {
                            ExprData::Assign { target, .. } => *target,
                            _ => value,
                        };
                        // Only a value with a name. A literal's path is the type it produces, so
                        // every `1` in the file would be recorded as one value handed to every
                        // call that took any of them -- and each read of one would then walk them
                        // all. See `ValuePath::names_a_value`.
                        if let Some(mut path) = path_of(hir, value).filter(ValuePath::names_a_value)
                        {
                            path.may_be_absent = false;
                            let statements = passed.entry(path).or_default();
                            if statements.last() != Some(&statement) {
                                statements.push(statement);
                            }
                        }
                    }
                },
            );
        }
        for statements in passed.values_mut() {
            statements.sort_unstable();
            statements.dedup();
        }
        Self(passed)
    }

    /// The statements whose call may pass `path`; empty for a path no call is handed.
    pub fn statements(&self, path: &ValuePath) -> &[StmtId] {
        let mut key = path.clone();
        key.may_be_absent = false;
        self.0.get(&key).map_or(&[], Vec::as_slice)
    }
}

/// The receiver of a method call whose name `leaves_receiver` admits, when it is a value that
/// keeps what a call leaves in it.
///
/// A local, `$this`, or a property of one of them. A receiver reached through a call is not one:
/// `$repo->items()->transform(...)` changed whatever `items()` handed back that time, and the next
/// `$repo->items()` may be another object altogether -- so reading it as the same slot would carry
/// one call's answer onto a different value.
fn receiver_left_changed(
    hir: &FileHir,
    expr: ExprId,
    leaves_receiver: &dyn Fn(&str) -> bool,
) -> Option<ValuePath> {
    let ExprData::MethodCall {
        receiver,
        name: Some(method),
        ..
    } = hir.expr(expr)
    else {
        return None;
    };
    if !leaves_receiver(method.as_str()) {
        return None;
    }
    let path = path_of(hir, *receiver)?;
    let keeps = path
        .steps
        .iter()
        .all(|step| matches!(step, crate::path::PathStep::Property(_)))
        && match path.root {
            PathRoot::Local(_) | PathRoot::This => true,
            // A static property: `Registry::$items->transform(...)`.
            PathRoot::Class(_) => !path.steps.is_empty(),
            _ => false,
        };
    keeps.then_some(path)
}

/// The entries one call contributes, shared by the narrow walk and the wide one.
///
/// One copy, because two would be the shape this file already warns about: the walks differ in
/// *which* calls they reach and in nothing else.
fn collect_passed(
    hir: &FileHir,
    expr: ExprId,
    subject: &ValuePath,
    leaves_receiver: &dyn Fn(&str) -> bool,
    checks: &mut Vec<PassedTo>,
) {
    let expr = &expr;
    let (callee, args) = match hir.expr(*expr) {
        ExprData::MethodCall {
            receiver,
            name: Some(method),
            args,
            ..
        } => match path_of(hir, *receiver) {
            Some(receiver) => (
                Callee::Member {
                    receiver,
                    name: method.clone(),
                },
                args,
            ),
            None => return,
        },
        ExprData::StaticCall {
            class,
            name: Some(method),
            args,
            ..
        } => (
            Callee::Member {
                receiver: ValuePath::of(crate::path::class_path_root(*class)),
                name: method.clone(),
            },
            args,
        ),
        // A free function, which is where both a by-reference parameter and a project's
        // own `assertFoo()` helper are most often written.
        ExprData::FunctionCall { name, args, .. } => (Callee::Function(*name), args),
        // `new Foo($out)` -- a constructor takes a by-reference parameter exactly as a method
        // does, and `passed_by_reference` already binds the name off one. Without this arm the
        // binding existed and the reader, walking the calls that passed the value, found none.
        ExprData::New { class, args, .. } => (Callee::Constructed(*class), args),
        _ => return,
    };

    let class_arguments = hir
        .expr_list(*args)
        .iter()
        .map(|&passed| class_constant_of(hir, passed))
        .collect::<Vec<_>>();

    // The receiver first, because PHP evaluates it first.
    if let Some(receiver) = receiver_left_changed(hir, *expr, leaves_receiver)
        && same_slot(&receiver, subject)
    {
        checks.push(PassedTo {
            at: hir.expr(*expr).range().start(),
            callee: callee.clone(),
            passed_as: PassedAs::Receiver(CallSource::of(*expr)),
            name: None,
            class_arguments: class_arguments.clone(),
        });
    }

    for (argument, &passed) in hir.expr_list(*args).iter().enumerate() {
        // A named argument wraps what was passed; the value inside is what the check is
        // about, and the name is which parameter it filled.
        let (name, passed) = match hir.expr(passed) {
            ExprData::Argument { name, value, .. } => (name.clone(), *value),
            _ => (None, passed),
        };
        // An assignment passed in place hands over the value it wrote, and writes it first: the
        // call is about the target from its end on, which is after the write it holds.
        // `assertInstanceOf(Pivot::class, $fresh = $pivot->fresh())` is the shape.
        let assigned = matches!(hir.expr(passed), ExprData::Assign { .. });
        if Subject::Expression(subject).wrote(hir, passed) {
            let range = hir.expr(*expr).range();
            checks.push(PassedTo {
                at: match assigned {
                    true => range.end(),
                    false => range.start(),
                },
                callee: callee.clone(),
                passed_as: PassedAs::Argument(argument),
                name: name.clone(),
                class_arguments: class_arguments.clone(),
            });
        }
    }
}

/// Where to look for a boolean that was written down before it was tested.
///
/// `$isBulk = $app instanceof BulkOrderApplication;` and then, twenty lines later,
/// `if ($isBulk)`. A condition tested in several places gets written down once, and a report
/// generator is nothing but that shape -- one test at the top and a dozen uses below it.
#[derive(Clone, Copy)]
struct Flags<'a> {
    scope: &'a Scope,
    /// The guard's own position. An assignment after it has not run yet.
    at: TextSize,
    /// `$a = $b; $b = $a;` is not code anyone means, but it is text an editor sees mid-edit, and a
    /// language server that hangs on it is worse than one that gives up.
    budget: u8,
    /// Whether a called function promises something about an argument. See [`AssertsArguments`].
    ///
    /// Carried here rather than passed beside the other three, because the guard reader recurses
    /// through `&&`, `!`, a strict comparison against `false` and a flag written down earlier --
    /// and this is needed at the bottom of all four. Whatever `proved_about` already reaches is
    /// where it belongs.
    asserts: AssertsArguments<'a>,
}

/// One assignment of a flag: the value it wrote down, and where the assignment ends.
#[derive(Clone, Copy)]
struct FlagWrite {
    value: ExprId,
    end: TextSize,
}

/// What a flag's reach is judged by. `return`, `throw` and `exit` leave; a call declared `never`
/// is read as falling through, because this layer cannot tell and [`Flags`] does not carry the
/// reader's answer. Wrong only in the direction that drops a claim: a write in a branch that ends
/// in `fail()` counts as one that may have run, which costs a finding and never hides one.
const LEAVING_BY_SYNTAX: NeverReturns<'static> = &|_, _| false;

impl Flags<'_> {
    /// The assignment of a flag that decides what testing it at `positive` proves.
    ///
    /// From the body's write sites rather than a walk of its statements: this is asked for every
    /// guard evaluated at every position, and the walk was the largest cost left in a diagnostics
    /// publish once the reach walks had gone.
    ///
    /// Newest first through the writes that may reach the guard, back to the first that certainly
    /// does -- it replaced everything older. Judged by reach and not by position alone, which is
    /// what the latest-before-the-guard scan got wrong: `if ($c) { $a = $x; } else { $a = $item
    /// !== null; }` ends with the second write and tests true just as readily after the first, and
    /// reading the second proved a non-null the code never checked.
    ///
    /// A write of the literal that cannot test `positive` is passed over -- `$ok = false; if ($c)
    /// { $ok = $x instanceof T; }` tests true only where the second ran, which is how a flag is
    /// written conditionally. Any other second candidate holds a value no claim here was read
    /// from, so two of them prove nothing. Nor does a flag that may never have been written, at the
    /// polarity an unset name satisfies.
    fn written(self, hir: &FileHir, flag: &str, positive: bool) -> Option<FlagWrite> {
        let facts = crate::facts::BodyFacts::of(hir);
        let mut found = None;
        let mut certain = false;
        for site in facts
            .writes_rooted_at(self.scope.body(), flag)
            .rev()
            .filter(|site| site.range.end() <= self.at)
        {
            let Some(reach) = facts.reach_of(hir, site, self.at, LEAVING_BY_SYNTAX) else {
                continue;
            };
            // A compound write, or one through an element, leaves the flag holding something no
            // condition was read from.
            let ExprData::Assign { target, value, .. } = hir.expr(site.expr) else {
                return None;
            };
            if !matches!(hir.expr(*target), ExprData::Variable { name, .. } if name.as_str() == flag)
            {
                return None;
            }
            if !Self::never_tests(hir, *value, positive) {
                if found.is_some() {
                    return None;
                }
                found = Some(FlagWrite {
                    value: *value,
                    end: site.range.end(),
                });
            }
            if reach == crate::reach::Reach::Certain {
                certain = true;
                break;
            }
        }
        // Unwritten on some path, where the name is null and tests false.
        if !certain && !positive {
            return None;
        }
        found
    }

    /// Whether a flag holding `value` can never test `positive`: `false` and `null` are never true,
    /// and `true` is never false.
    fn never_tests(hir: &FileHir, value: ExprId, positive: bool) -> bool {
        match hir.expr(value) {
            ExprData::Literal { kind, .. } => match positive {
                true => matches!(kind, LiteralKind::False | LiteralKind::Null),
                false => matches!(kind, LiteralKind::True),
            },
            _ => false,
        }
    }

    /// Whether what `subject` names may have been replaced after a flag about it was written down.
    ///
    /// A flag keeps what its condition proved about the value the subject held *then*, while the
    /// claim is resolved where the flag is tested -- so `$ok = $item !== null; $item = $next;
    /// if ($ok)` proved `$next` not null. A write that only *may* have run counts as surely as one
    /// that did: what the subject holds at the guard is then the join of both, and the claim would
    /// narrow the half it was never about. A guard tests its subject where it stands and needs no
    /// such check; a flag is the one way a test outlives its subject.
    ///
    /// Assignments only, as for a guard. `??=` writes only where the value was null, which a claim
    /// that it is not null has already ruled out.
    fn rewritten(self, hir: &FileHir, subject: Subject<'_>, since: TextSize) -> bool {
        let facts = crate::facts::BodyFacts::of(hir);
        facts
            .writes_that_may_root_at(self.scope.body(), subject.written_root())
            .filter(|site| site.range.start() > since && site.range.end() <= self.at)
            .filter(|site| subject.may_be_written_by(site))
            .filter(|site| match hir.expr(site.expr) {
                ExprData::Assign { target, .. } | ExprData::CompoundAssign { target, .. } => {
                    subject.replaced_by(hir, *target)
                }
                _ => false,
            })
            .any(|site| {
                facts
                    .reach_of(hir, site, self.at, LEAVING_BY_SYNTAX)
                    .is_some()
            })
    }

    /// Whether the function this name resolves to promises something about an argument.
    fn asserts(self, hir: &FileHir, name: phrost_hir::NameRefId) -> bool {
        self.asserts.asserts(hir, name)
    }

    fn spend(self) -> Option<Self> {
        (self.budget > 0).then_some(Self {
            budget: self.budget - 1,
            ..self
        })
    }
}

/// How many boolean flags to follow through. Two is one more than anyone writes.
const FLAG_DEPTH: u8 = 2;

/// What a guard can be about.
///
/// Two shapes because they cost different amounts to check. A local is a name, compared against the
/// variable a guard names -- one string comparison per guard, and the overwhelmingly common case.
/// Anything else is compared as a whole path, which means reading the guard's subject as a path
/// first: `if ($this->users[$uid] instanceof User)` narrows the expression it names, and that
/// expression is the only thing in the file that says what a bare `array` holds.
#[derive(Clone, Copy)]
pub(crate) enum Subject<'a> {
    Local(&'a str),
    Expression(&'a ValuePath),
}

impl<'a> Subject<'a> {
    /// Whether a memo's key names this subject.
    pub(crate) fn is_key(self, key: &crate::proved_memo::SubjectKey) -> bool {
        match (self, key) {
            (Self::Local(name), crate::proved_memo::SubjectKey::Local(kept)) => name == &**kept,
            (Self::Expression(path), crate::proved_memo::SubjectKey::Path(kept)) => path == kept,
            _ => false,
        }
    }

    /// A hash of the subject, the same for every spelling [`Self::is_key`] would accept.
    pub(crate) fn hash_key(self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        match self {
            Self::Local(name) => (0_u8, name).hash(&mut hasher),
            Self::Expression(path) => (1_u8, path).hash(&mut hasher),
        }
        hasher.finish()
    }

    /// The subject as an owned key, for the memo.
    pub(crate) fn key(self) -> crate::proved_memo::SubjectKey {
        match self {
            Self::Local(name) => crate::proved_memo::SubjectKey::Local(name.into()),
            Self::Expression(path) => crate::proved_memo::SubjectKey::Path(path.clone()),
        }
    }

    /// Whether a write site could be about this subject, judged by the variable under its target:
    /// the cheap test that keeps [`Self::is`] from being asked about every write in the body.
    /// The variable [`Self::may_be_written_by`] tests a write's root against, if it tests one.
    fn written_root(self) -> Option<&'a str> {
        match self {
            Self::Local(name) => Some(name),
            Self::Expression(path) => match &path.root {
                PathRoot::Local(name) => Some(name.as_str()),
                PathRoot::This => Some("this"),
                _ => None,
            },
        }
    }

    fn may_be_written_by(self, site: &crate::facts::WriteSite) -> bool {
        match self {
            Self::Local(name) => site.rooted_at(name),
            Self::Expression(path) => match &path.root {
                PathRoot::Local(name) => site.rooted_at(name.as_str()),
                PathRoot::This => site.rooted_at("this"),
                _ => true,
            },
        }
    }

    /// Whether `expr` is the thing this subject refers to.
    fn is(self, hir: &FileHir, expr: ExprId) -> bool {
        match self {
            Self::Local(name) => {
                matches!(hir.expr(expr), ExprData::Variable { name: written, .. } if written.as_str() == name)
            }
            // A path built from two occurrences of the same expression compares equal as long as
            // nothing in it carries a syntax id -- which properties, elements, locals and `$this`
            // do not. A guard on `Foo::get()` therefore does not narrow a second `Foo::get()`, and
            // that is a miss rather than a wrong answer.
            Self::Expression(path) => {
                path_of(hir, expr).is_some_and(|written| same_slot(&written, path))
            }
        }
    }

    /// Whether assigning to `target` replaces what this subject names: the subject itself, or a
    /// value it is reached through -- `$h = $other` replaces `$h->source` as surely as writing the
    /// field does. A write *below* the subject, `$item->name = ''`, leaves it the same value.
    fn replaced_by(self, hir: &FileHir, target: ExprId) -> bool {
        match self {
            Self::Local(_) => self.is(hir, target),
            Self::Expression(path) => {
                path_of(hir, target).is_some_and(|written| crate::path::covers_slot(&written, path))
            }
        }
    }

    /// The same, looking through an assignment to the name it wrote.
    ///
    /// `if (($user = $this->getUser($email)) instanceof User)` is one expression doing two things,
    /// and what the code after it reads is `$user`. Only the *match* looks through: the path the
    /// claim carries stays the assignment's, for the reason the `ExprData::Assign` arm of
    /// [`proved_about`] gives -- asking what the name held at the guard would be asking from before
    /// the assignment ran.
    fn wrote(self, hir: &FileHir, expr: ExprId) -> bool {
        match hir.expr(expr) {
            ExprData::Assign { target, .. } => self.is(hir, *target),
            _ => self.is(hir, expr),
        }
    }

    /// This subject's own path, when `expr` is a value reached *through* it.
    ///
    /// `$row['vin']` for `$row`, `$this->row['vin']` for `$this->row`. Strictly through: a path
    /// equal to the subject's is [`Self::is`]'s answer and not this one.
    ///
    /// Asked only by the two callers reading a construct that answers for a whole *path* rather than
    /// reading it -- `isset` and `??`. No operator qualifies: `$row['vin'] !== null` had to read
    /// `$row` to get its operand, and is itself the site that would warn.
    fn reached_through(self, hir: &FileHir, expr: ExprId) -> Option<ValuePath> {
        let written = path_of(hir, expr)?;
        if written.steps.is_empty() {
            return None;
        }
        let mine = match self {
            Self::Local(name) => match &written.root {
                PathRoot::Local(local) if local.as_str() == name => {
                    ValuePath::of(written.root.clone())
                }
                _ => return None,
            },
            Self::Expression(path) => {
                if path.steps.len() >= written.steps.len()
                    || !crate::path::covers_slot(path, &written)
                {
                    return None;
                }
                (*path).clone()
            }
        };
        // `$this` is never null, so `isset($this->rows['a'])` proving it is not says nothing -- and
        // a claim about `$this` is not free: resolving one asks what `$this` was narrowed to, which
        // asks the guards again. Every other root here is a value that can be null.
        (mine.root != PathRoot::This || !mine.steps.is_empty()).then_some(mine)
    }
}

/// The class a condition asserts `subject` has, at the given polarity.
///
/// Polarity is what lets a guard be read backwards. `if (!($x instanceof T)) { return; }` proves `T`
/// for the code after it, and getting there means asking what the condition proves when it is
/// *false* — which is the same walk with the sign flipped.
///
/// De Morgan decides which connective carries an contract at which polarity. When the condition
/// holds, both halves of an `&&` hold and neither half of an `||` need; when it does not, both
/// halves of an `||` fail and neither half of an `&&` need.
/// What a guard proved about a value.
///
/// Three answers rather than one, because only the first names a class. A truthiness test does not
/// say what the value *is* -- something else already said that -- it says which of the things it
/// could be are now ruled out, and only the layer that knows the type can turn that into an answer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Proved {
    /// `$x instanceof T`, or an `assert` of one.
    ///
    /// Carries the path, because what the guard proves is not the whole of what the value is: a
    /// declaration that named something unrelated still holds, and the value is then both.
    Class { of: ValuePath, class: ClassTarget },
    /// `if ($x)`, `if ($x !== null)`, `isset($x)`. Whatever the value was, it is not null.
    ///
    /// Carries the path because resolving it means asking about that value again, from before the
    /// guard, and a claim with no class in it does not say which value.
    ///
    /// `refused` says which of PHP's three groups of test it was; see
    /// [`PathRoot::NotNull`](crate::PathRoot::NotNull), where the difference is spelled out.
    NotNull { of: ValuePath, refused: Refused },
    /// `$x !== ''`, and the else of `$x === ''` -- the value is not the empty string, and nothing
    /// is said about null. See [`PathRoot::NotEmptyString`](crate::PathRoot::NotEmptyString).
    NotEmptyString {
        of: ValuePath,
        /// What the strings are known to hold at least as much as -- see the root.
        like: Option<ExprId>,
    },
    /// `if (!$x)`, `if ($x === null)`, `empty($x)` -- and the other side of every guard above.
    ///
    /// `strictly` when the test named null rather than falsehood; see
    /// [`PathRoot::Null`](crate::PathRoot::Null).
    Null { of: ValuePath, strictly: bool },
    /// `if (!($x instanceof T))`, and the else of every `instanceof` guard.
    ///
    /// `exactly` when only `T` itself was ruled out and not what extends it: `$x::class !==
    /// T::class`. That says nothing unless `T` is final, which only the type layer knows.
    NotClass {
        of: ValuePath,
        class: ClassTarget,
        exactly: bool,
    },
    /// `if ($x instanceof A || $x instanceof B)` -- the value is one of them.
    ///
    /// Several, and not one repeated: a single `instanceof` narrows what the declaration said and
    /// may compose with it, while several say only which of the declared alternatives are still
    /// possible -- and which those are is a question about what the declaration listed.
    OneOfClasses {
        of: ValuePath,
        classes: Vec<ClassTarget>,
    },
    /// Several claims about one value, any of which may be the one that held.
    ///
    /// The general form [`Self::OneOfClasses`] is one case of: that one is every operand being an
    /// `instanceof`, and this is every operand being *anything about the same value*.
    /// `is_string($x) || $x instanceof Foo` proves `string|Foo` and neither operand proves it
    /// alone, and `!is_string($x) && !$x instanceof Foo` proves the same union past a `throw` --
    /// the negation of a conjunction of negations is the disjunction, which is how an early-return
    /// guard with two alternatives is written.
    ///
    /// Kept beside `OneOfClasses` rather than replacing it. That one asks *which of the
    /// declaration's alternatives are left*, which is a question about the declared union; this
    /// resolves each operand's own narrowing and unions the answers. Both are right and they are
    /// not the same reading -- see `OneOfClasses` for why several `instanceof` need the first.
    EitherOf(Vec<Proved>),
    /// `if (is_string($x))` and the rest of PHP's type predicates.
    Kind {
        of: ValuePath,
        kind: phrost_model::BuiltinTy,
        holds: bool,
        /// Whether the test *names* the kind, or only rules some alternatives of it out.
        ///
        /// `is_string($x)` names it: on the branch that passed, the value is a string whatever the
        /// declaration said, so the guard is the whole answer where the declaration said something
        /// else. `!in_array($x, ['a', 'b'], true)` does not -- it rules two values out, and reading
        /// that as "an object" is sound only as a filter over alternatives a declaration already
        /// listed. See [`in_array_of_non_objects`], which is the one claim here written that way.
        settled: bool,
    },
    /// A test that held and names no single kind: which sentinels it ruled out, and the kinds no
    /// value passing it can be. `is_numeric($x)` leaves no null, `false`, object, array or
    /// callable. See [`RulesOut`].
    ///
    /// One claim and not a `NotNull` with a `Kind` per kind nested on it, because each claim
    /// nested in another is a step of the reader's budget, and seven of them would spend half of
    /// it on one guard -- and because the path that skipped a write is described by the outermost
    /// claim alone, which would have kept one kind and forgotten the sentinels.
    NoneOfKinds {
        of: ValuePath,
        refused: Refused,
        kinds: &'static [phrost_model::BuiltinTy],
    },
    /// `is_string($box->value)` about `$box`: which of the classes the value may be have a
    /// property of that kind. A union of classes told apart by what a property holds, read the way
    /// a tagged union is. See [`PathRoot::MemberKind`](crate::PathRoot::MemberKind).
    MemberKind {
        of: ValuePath,
        property: phrost_model::Name,
        kind: phrost_model::BuiltinTy,
        holds: bool,
    },
    /// `array_key_exists($key, $map)` and `isset($map[$key])` that held: the key is one the
    /// container holds, and so of the kind its keys are. Only the side that held -- a key that is
    /// absent may be of any kind. See [`PathRoot::KeyOf`](crate::PathRoot::KeyOf).
    KeyOf { of: ValuePath, container: ValuePath },
    /// `in_array($x, $list, true)` that held: the value is one of the container's elements. Only
    /// the side that held and only strict -- see [`element_test`] and
    /// [`PathRoot::ElementOf`](crate::PathRoot::ElementOf).
    ElementOf { of: ValuePath, container: ValuePath },
    /// `$x !== [null]`, and the else of `$x === [null]`: the value is not that array. What it rules
    /// out of each slot depends on what the value's other slots hold, which is the type layer's to
    /// say -- see [`PathRoot::NotIdenticalTo`](crate::PathRoot::NotIdenticalTo).
    NotIdenticalTo { of: ValuePath, literal: ExprId },
    /// `$x === Status::Draft` that held, and `$order?->status === Status::Draft` about `$order`:
    /// whichever sentinel `other` cannot be, `of` is not -- see
    /// [`PathRoot::SentinelsRefusedBy`](crate::PathRoot::SentinelsRefusedBy).
    SentinelsRefusedBy { of: ValuePath, other: ValuePath },
    /// `if ($x->isSomething())`, where the method's docblock says what that proves.
    Asserted {
        receiver: ValuePath,
        method: phrost_model::Name,
        when_true: bool,
    },
    /// `if (Validator::isUser($x))`, `if ($validator->isUser($x))` -- a boolean call in a guard
    /// whose tag names one of its own *arguments* rather than the receiver it was called on.
    ///
    /// The mirror of [`Self::Asserted`]: there the call's own receiver is the value being narrowed
    /// and the class comes off it; here the call is on a different class and the narrowed value is
    /// argument `argument` instead. So this carries what was called and which position, not a
    /// class -- the reader resolves the callee's declaration to find the tag, the same lookup
    /// [`PathRoot::ByReference`](crate::PathRoot::ByReference) already needs for a written-through
    /// parameter. Not produced for a free function -- see
    /// [`asserted_argument_of`] and [`PathRoot::AssertedArgument`](crate::PathRoot::AssertedArgument).
    ///
    /// `when_true` here is the branch's own polarity, captured as `positive` was at the call site
    /// -- not the tag's. The two are only compared against each other once the tag itself is read,
    /// at resolution.
    AssertedArgument {
        of: ValuePath,
        callee: Callee,
        argument: usize,
        /// The parameter it was written under, for `f(value: $x)`. `None` for a positional
        /// argument.
        name: Option<phrost_model::Name>,
        when_true: bool,
    },
}

impl Proved {
    /// Two claims about one value, from one condition that proves both.
    ///
    /// An contract is *composed* rather than ranked, because what it proves is written on the method
    /// -- in another file, which this layer does not read. So `$x && $x->isSomething()` keeps both
    /// halves: the contract sits on top of the certain claim, and the layer that reads the tag uses
    /// it if there is one and keeps what was underneath if there is not.
    ///
    /// Ranking them instead was wrong in both directions, and both were measured on a real project.
    /// Preferring the contract lost the null-safety of `if ($this->offers &&
    /// $this->offers->hasAcceptedQuote())` for every untagged method -- 23 new findings inside written
    /// guards. Preferring the certain claim loses the subclass on
    /// `if ($app && $app->isBulkOrder())`, which is how eight methods of one Drupal class are
    /// used. Composing needs neither choice.
    ///
    /// Everything else nests, for the reason [`Self::about`] gives: both claims hold, so the second
    /// is about the value the first left. Ranking them instead dropped whichever was weaker, and
    /// `if ($value && !is_array($value))` is written by an author who has just ruled the null out --
    /// so a claim about the *kind* outranking one about the null threw away the null-safety they
    /// wrote.
    ///
    /// The one pair that still ranks is a claim that the value *is* null beside one that leaves a
    /// value. One condition proving both is contradictory and the code will answer for it; the
    /// useful reading is the one that leaves something to work with.
    fn and_also(self, other: Self, at: TextSize) -> Self {
        match (self, other) {
            // Either order, and two contracts nest the same way -- the outer one is asked about a
            // receiver the inner one already refined.
            //
            // `under` is first re-anchored on what the contract's receiver already carried, and the
            // contract goes on top of that. Replacing the receiver with `under` outright threw the
            // receiver away, and a receiver is where an earlier composition lives: in
            // `!$a || !$a->isPending() || !$this->m->isEditAllowed($a)` the first two operands
            // had composed into an `Asserted` whose receiver was `NotNull($a)`, and composing the
            // third put it on a bare `$a` -- so a three-operand guard proved less than a
            // two-operand one, and the null the first operand ruled out came back.
            (
                under,
                Self::Asserted {
                    receiver,
                    method,
                    when_true,
                },
            )
            | (
                Self::Asserted {
                    receiver,
                    method,
                    when_true,
                },
                under,
            ) => Self::Asserted {
                receiver: ValuePath::of(under.anchored_on(receiver).root(at)),
                method,
                when_true,
            },
            // The argument-form contract nests the same way, on its own `of` rather than on a
            // receiver: `$x && Validator::isUser($x)` composes `NotNull($x)` with the assertion by
            // re-anchoring `of` to what the first half already narrowed.
            (
                under,
                Self::AssertedArgument {
                    of,
                    callee,
                    argument,
                    name,
                    when_true,
                },
            )
            | (
                Self::AssertedArgument {
                    of,
                    callee,
                    argument,
                    name,
                    when_true,
                },
                under,
            ) => Self::AssertedArgument {
                of: ValuePath::of(under.anchored_on(of).root(at)),
                callee,
                argument,
                name,
                when_true,
            },
            // Two classes ruled out, and both hold: `!($x instanceof A || $x instanceof B)` is
            // `!A && !B`. Nested rather than ranked, the way an assertion is -- the outer claim is
            // about a value the inner one has already narrowed, and taking whichever was stronger
            // would rule out one class and forget the other.
            (inner @ Self::NotClass { .. }, Self::NotClass { class, exactly, .. }) => {
                Self::NotClass {
                    of: ValuePath::of(inner.root(at)),
                    class,
                    exactly,
                }
            }
            (kept @ Self::Null { .. }, Self::Null { .. }) => kept,
            (Self::Null { .. }, kept) | (kept, Self::Null { .. }) => kept,
            (inner, outer) => outer.about(ValuePath::of(inner.root(at))),
        }
    }

    /// This claim, made about `of` -- unless it already is, through the claims it nests.
    ///
    /// `$x === null || $x === '' || f($x)` failing composes the first two into a claim about `''`
    /// nested over one about null, both about `$x`; the third is a contract on `$x` too. Re-anchored
    /// with [`Self::about`], the nest was cut at its outermost claim and the null came back --
    /// which a guard of two operands never showed, since a lone claim has nothing nested to lose.
    fn anchored_on(self, of: ValuePath) -> Self {
        match self.subject().map(innermost) == Some(&of) {
            true => self,
            false => self.about(of),
        }
    }

    /// The value this claim is about, as its own path says -- which may be another claim's root.
    fn subject(&self) -> Option<&ValuePath> {
        match self {
            Self::Class { of, .. }
            | Self::NotNull { of, .. }
            | Self::NotEmptyString { of, .. }
            | Self::Null { of, .. }
            | Self::NotClass { of, .. }
            | Self::OneOfClasses { of, .. }
            | Self::Kind { of, .. }
            | Self::NoneOfKinds { of, .. }
            | Self::MemberKind { of, .. }
            | Self::KeyOf { of, .. }
            | Self::ElementOf { of, .. }
            | Self::NotIdenticalTo { of, .. }
            | Self::SentinelsRefusedBy { of, .. }
            | Self::AssertedArgument { of, .. } => Some(of),
            Self::Asserted { receiver, .. } => Some(receiver),
            Self::EitherOf(_) => None,
        }
    }

    /// The same claim, about a value another claim has already narrowed.
    ///
    /// Nesting rather than ranking, because two guards are not two readings of one condition: `if
    /// (A) return; elseif (B) return;` leaves neither an A nor a B, and keeping whichever claim was
    /// stronger would rule one out and forget the other. That is the difference from
    /// [`Self::and_also`], which reads one condition that proves two things.
    ///
    /// A contract is re-anchored the same way: what it proves is still looked up on the method at
    /// resolution, and the receiver it is looked up *for* is the value the other claim narrowed --
    /// `if (!$a) return; elseif (!$a->isX()) return;` asks `isX`'s tag about a non-null `$a`.
    fn about(self, of: ValuePath) -> Self {
        match self {
            Self::Class { class, .. } => Self::Class { of, class },
            Self::NotClass { class, exactly, .. } => Self::NotClass { of, class, exactly },
            Self::OneOfClasses { classes, .. } => Self::OneOfClasses { of, classes },
            // Each operand re-anchored, because the disjunction is about whatever the other claim
            // narrowed: `if (!$a) return; elseif (!is_string($a) && !$a instanceof Foo) return;`
            // asks the union of a non-null `$a`.
            Self::EitherOf(operands) => Self::EitherOf(
                operands
                    .into_iter()
                    .map(|operand| operand.about(of.clone()))
                    .collect(),
            ),
            Self::Kind {
                kind,
                holds,
                settled,
                ..
            } => Self::Kind {
                of,
                kind,
                holds,
                settled,
            },
            Self::NoneOfKinds { refused, kinds, .. } => Self::NoneOfKinds { of, refused, kinds },
            Self::KeyOf { container, .. } => Self::KeyOf { of, container },
            Self::ElementOf { container, .. } => Self::ElementOf { of, container },
            Self::NotIdenticalTo { literal, .. } => Self::NotIdenticalTo { of, literal },
            Self::SentinelsRefusedBy { other, .. } => Self::SentinelsRefusedBy { of, other },
            Self::MemberKind {
                property,
                kind,
                holds,
                ..
            } => Self::MemberKind {
                of,
                property,
                kind,
                holds,
            },
            Self::NotNull { refused, .. } => Self::NotNull { of, refused },
            Self::NotEmptyString { like, .. } => Self::NotEmptyString { of, like },
            Self::Null { strictly, .. } => Self::Null { of, strictly },
            Self::Asserted {
                method, when_true, ..
            } => Self::Asserted {
                receiver: of,
                method,
                when_true,
            },
            Self::AssertedArgument {
                callee,
                argument,
                name,
                when_true,
                ..
            } => Self::AssertedArgument {
                of,
                callee,
                argument,
                name,
                when_true,
            },
        }
    }

    /// The path root this becomes.
    ///
    /// `at` is where the guard was written, and the two truthiness roots carry it because resolving
    /// them means asking what the value was *before* the guard -- which is safe from there, because
    /// a guard does not reach its own position.
    pub(crate) fn root(self, at: TextSize) -> PathRoot {
        match self {
            Self::Class { of, class } => PathRoot::Narrowed {
                of: Box::new(of),
                at,
                class,
            },
            Self::NotNull { of, refused } => PathRoot::NotNull {
                of: Box::new(of),
                at,
                refused,
            },
            Self::NotEmptyString { of, like } => PathRoot::NotEmptyString {
                of: Box::new(of),
                at,
                like,
            },
            Self::Null { of, strictly } => PathRoot::Null {
                of: Box::new(of),
                at,
                strictly,
            },
            Self::NotClass { of, class, exactly } => PathRoot::NotOfClass {
                of: Box::new(of),
                at,
                class,
                exactly,
            },
            Self::OneOfClasses { of, classes } => PathRoot::OneOfClasses {
                of: Box::new(of),
                at,
                classes,
            },
            // Each operand's own root, read at the same position, and the union is the root the
            // view already knows how to resolve -- one value out of several, every operand of which
            // has to answer or the whole thing does.
            Self::EitherOf(operands) => PathRoot::EitherOf {
                operands: operands
                    .into_iter()
                    .map(|proved| crate::path::OperandSite {
                        value: ValuePath::of(proved.root(at)),
                        at,
                    })
                    .collect(),
                unless: crate::path::Unless::Nothing,
            },
            Self::Kind {
                of,
                kind,
                holds,
                settled,
            } => PathRoot::OfKind {
                of: Box::new(of),
                at,
                kind,
                holds,
                settled,
            },
            Self::NoneOfKinds { of, refused, kinds } => PathRoot::NoneOfKinds {
                of: Box::new(of),
                at,
                refused,
                kinds,
            },
            Self::KeyOf { of, container } => PathRoot::KeyOf {
                of: Box::new(of),
                at,
                container: Box::new(container),
            },
            Self::ElementOf { of, container } => PathRoot::ElementOf {
                of: Box::new(of),
                at,
                container: Box::new(container),
            },
            Self::NotIdenticalTo { of, literal } => PathRoot::NotIdenticalTo {
                of: Box::new(of),
                at,
                literal,
            },
            Self::SentinelsRefusedBy { of, other } => PathRoot::SentinelsRefusedBy {
                of: Box::new(of),
                at,
                other: Box::new(other),
            },
            Self::MemberKind {
                of,
                property,
                kind,
                holds,
            } => PathRoot::MemberKind {
                of: Box::new(of),
                at,
                property,
                kind,
                holds,
            },
            Self::Asserted {
                receiver,
                method,
                when_true,
            } => PathRoot::Asserted {
                receiver: Box::new(receiver),
                method,
                when_true,
                at,
            },
            Self::AssertedArgument {
                of,
                callee,
                argument,
                name,
                when_true,
            } => PathRoot::AssertedArgument {
                of: Box::new(of),
                callee: Box::new(callee),
                argument,
                name,
                when_true,
                at,
            },
        }
    }
}

/// What a disjunction of `instanceof` proves, in either of the two ways PHP writes one.
///
/// `$x instanceof A || $x instanceof B` says so when it *holds*. Its De Morgan twin --
/// `!$x instanceof A && !$x instanceof B` -- says the same thing when it does **not**, and that is
/// the dominant shape in real code, because it is how an early-return guard with two alternatives is
/// written:
///
/// ```php
/// if (!$zero instanceof One && !$zero instanceof Two) {
///     return;
/// }
/// $zero->foo();
/// ```
///
/// One function rather than two, because they are one claim: the negation of a conjunction of
/// negations is the disjunction. Writing the second as its own collector would be the two-
/// constructors-of-one-rule shape this file already warns about, and the two would drift.
///
/// Every operand has to be one, in both spellings. A single operand that is something else --
/// `|| $other`, or `&& !$other` -- means the branch can be reached without any `instanceof` having
/// decided it, and then nothing is proved at all.
fn one_of_instanceof(
    hir: &FileHir,
    condition: ExprId,
    subject: Subject<'_>,
    joined_by: LogicalOp,
    negated: bool,
) -> Option<Proved> {
    let mut of = None;
    let mut classes = Vec::new();
    collect_instanceof(
        hir,
        condition,
        subject,
        joined_by,
        negated,
        &mut of,
        &mut classes,
    )?;
    // Two or more, because one is a plain `instanceof` and `Proved::Class` says more about it: what
    // the value is, rather than which alternatives are left.
    if classes.len() < 2 {
        return None;
    }
    Some(Proved::OneOfClasses { of: of?, classes })
}

/// Every leaf of a disjunction, when each is a claim about `subject`.
///
/// The general form of [`one_of_instanceof`], and it runs after that one rather than in place of
/// it: several `instanceof` are answered better by `OneOfClasses`, which asks which of the
/// declaration's alternatives survive rather than unioning what each operand narrows to.
///
/// Both spellings, for the reason the instanceof collector reads both: `A || B` holding and
/// `!A && !B` *not* holding are the same claim, and the second is how an early-return guard with
/// two alternatives is written.
///
/// **Every leaf has to answer.** An operand proving nothing about this value means the branch can
/// be reached without any of the others having decided it, so a union of the ones that did answer
/// would be narrower than the truth -- the rule `list_literal_element` states about entries, here
/// about alternatives.
fn either_of_claims(
    hir: &FileHir,
    condition: ExprId,
    subject: Subject<'_>,
    joined_by: LogicalOp,
    flags: Flags,
) -> Option<Proved> {
    // A disjunction read at the polarity that makes it one: `||` where the branch is taken, and
    // `&&` where it is not, which is the caller's `!carries`.
    let negated = matches!(joined_by, LogicalOp::And);
    let mut claims = Vec::new();
    collect_claims(
        hir,
        condition,
        subject,
        joined_by,
        negated,
        flags,
        &mut claims,
    )?;
    // Two or more: one operand is a plain guard, and every rule above this already reads those.
    if claims.len() < 2 {
        return None;
    }
    Some(Proved::EitherOf(claims))
}

/// Walks the operator's own tree, asking each leaf what it proves about `subject`.
fn collect_claims(
    hir: &FileHir,
    condition: ExprId,
    subject: Subject<'_>,
    joined_by: LogicalOp,
    negated: bool,
    flags: Flags,
    out: &mut Vec<Proved>,
) -> Option<()> {
    if let ExprData::Logical {
        operator,
        left,
        right,
        ..
    } = hir.expr(condition)
        && *operator == joined_by
    {
        collect_claims(hir, *left, subject, joined_by, negated, flags, out)?;
        return collect_claims(hir, *right, subject, joined_by, negated, flags, out);
    }
    // `negated` is the `!A && !B` spelling: each leaf is read at the opposite polarity, which is
    // what turns the conjunction of negations back into the disjunction it means.
    out.push(proved_about(hir, condition, subject, !negated, flags)?);
    Some(())
}

/// Walks a tree joined by one operator, requiring every leaf to be an `instanceof` about `subject`.
///
/// `negated` says whether each leaf is written under a `!`, which is what makes this serve both
/// spellings above. It is not a per-leaf choice: a tree mixing `!$x instanceof A` with a bare
/// `$x instanceof B` under `&&` says something this cannot express, so a leaf of the wrong shape
/// declines the whole thing.
fn collect_instanceof(
    hir: &FileHir,
    condition: ExprId,
    subject: Subject<'_>,
    joined_by: LogicalOp,
    negated: bool,
    of: &mut Option<ValuePath>,
    classes: &mut Vec<ClassTarget>,
) -> Option<()> {
    match hir.expr(condition) {
        ExprData::Logical {
            operator,
            left,
            right,
            ..
        } if *operator == joined_by => {
            collect_instanceof(hir, *left, subject, joined_by, negated, of, classes)?;
            collect_instanceof(hir, *right, subject, joined_by, negated, of, classes)
        }
        // `!$x instanceof A` parses as `!($x instanceof A)`, because `instanceof` binds tighter.
        ExprData::Not { value, .. } if negated => {
            let ExprData::Instanceof { value, class, .. } = hir.expr(*value) else {
                return None;
            };
            collect_one(hir, *value, *class, subject, of, classes)
        }
        ExprData::Instanceof { value, class, .. } if !negated => {
            collect_one(hir, *value, *class, subject, of, classes)
        }
        _ => None,
    }
}

/// One `instanceof` leaf, recorded when it is about the subject and refused when it is not.
fn collect_one(
    hir: &FileHir,
    value: ExprId,
    class: ClassTarget,
    subject: Subject<'_>,
    of: &mut Option<ValuePath>,
    classes: &mut Vec<ClassTarget>,
) -> Option<()> {
    if !subject.wrote(hir, value) {
        return None;
    }
    if of.is_none() {
        *of = path_of(hir, value);
    }
    classes.push(class);
    Some(())
}

/// What a test that names no single kind proves when it holds.
///
/// All three are false for null. What separates them is `false`, and the answer is per predicate
/// rather than per group: `is_numeric(FALSE)` and `is_countable(FALSE)` are both false, so the
/// sentinel is gone -- and `is_scalar(FALSE)` is **true**, because a bool is a scalar, so it is not.
/// The three were one answer here, and `array_search($id, $keys)` under `if (is_numeric($index))`
/// kept a `false` the guard had ruled out.
///
/// None of them is a truthiness test -- `is_numeric('0')` is true and `'0'` is falsy -- which is
/// what [`Refused::TheSentinels`] exists to say and what reading them as `if ($x)` would get wrong.
///
/// What each proves beyond the sentinels is a union this vocabulary has no word for, so it is
/// carried as the kinds it rules out -- a filter over what the declaration listed, never a claim of
/// its own. `is_numeric($callback)` on Laravel's `callable|int|null` is the int, because no
/// callable is numeric: a function's name cannot start with a digit, and the other two spellings
/// of one are an array and an object. A kind is listed only where *no* value of it passes:
/// `is_scalar` keeps `callable`, since `'strlen'` is both, and `is_countable` keeps it too, since
/// `[$object, 'method']` is both.
///
/// Three, and not a longer list of everything that happens to reject null: a test whose useful half
/// names one kind belongs in [`type_predicate`] instead, where it says more. And a length tested for
/// its truth, which rules out the empty string the way `strlen($s) > 0` does.
struct RulesOut {
    refused: Refused,
    kinds: &'static [phrost_model::BuiltinTy],
}

impl RulesOut {
    fn of(written: &str) -> Option<Self> {
        use phrost_model::BuiltinTy::{
            Array, Bool, Callable, Float, Int, Iterable, Object, Resource, String,
        };

        Some(
            match written
                .trim_start_matches('\\')
                .to_ascii_lowercase()
                .as_str()
            {
                // And not `''`, which spells no number: a string that passes holds something.
                "is_numeric" => Self {
                    refused: Refused::TheEmptyString,
                    kinds: &[Object, Array, Iterable, Callable, Bool, Resource],
                },
                // A length that is not zero, read as the truth it is in `if (strlen($s))`. Null and
                // `false` measure nothing and `''` is the one string that does; an array is a
                // `TypeError`, so no array gets past.
                "strlen" | "mb_strlen" => Self {
                    refused: Refused::TheEmptyString,
                    kinds: &[Array],
                },
                "is_scalar" => Self {
                    refused: Refused::NullOnly,
                    kinds: &[Object, Array, Iterable, Resource],
                },
                "is_countable" => Self {
                    refused: Refused::TheSentinels,
                    kinds: &[String, Int, Float, Bool, Resource],
                },
                _ => return None,
            },
        )
    }
}

/// The kind of value a PHP type predicate tests for: the ones that name a single type and nothing
/// else. A test whose truth spans several is [`RulesOut`]'s.
fn type_predicate(written: &str) -> Option<phrost_model::BuiltinTy> {
    use phrost_model::BuiltinTy;

    // Written `\is_string` as often as not, and PHP compares function names without regard to case.
    let name = written.trim_start_matches('\\').to_ascii_lowercase();
    Some(match name.as_str() {
        "is_string" => BuiltinTy::String,
        "is_int" | "is_integer" | "is_long" => BuiltinTy::Int,
        "is_float" | "is_double" => BuiltinTy::Float,
        "is_bool" => BuiltinTy::Bool,
        "is_array" => BuiltinTy::Array,
        "is_object" => BuiltinTy::Object,
        "is_callable" => BuiltinTy::Callable,
        "is_iterable" => BuiltinTy::Iterable,
        "is_null" => BuiltinTy::Null,
        // Added with the kind itself: a type nothing can guard is a type that reports every
        // guarded use, which is how a precision gain turns into noise.
        "is_resource" => BuiltinTy::Resource,
        _ => return None,
    })
}

/// An argument as the expression it passes, looking through the wrapper a named or spread
/// argument gets.
///
/// `None` for a spread: `f(...$rest)` passes however many arguments are in `$rest`, and none of
/// them is the one written at this position.
fn positional_argument(hir: &FileHir, argument: ExprId) -> Option<ExprId> {
    match hir.expr(argument) {
        ExprData::Argument {
            value,
            spread: false,
            ..
        } => Some(*value),
        ExprData::Argument { .. } => None,
        _ => Some(argument),
    }
}

/// `in_array($x, [<values none of which is an object>], true)`, strict membership against a
/// haystack the file wrote as scalars and class constants.
///
/// A discriminator exactly like `is_string` is, and read the same way: proving the value is *not*
/// one of the haystack's kind is the useful half, and it is what turns `if (in_array($val, [...],
/// true)) { return; } $val->` from nothing into whichever class-typed alternative a declared union
/// left. `BuiltinTy::Object` rather than the haystack's own scalar kind, because nothing here
/// resolves what a class constant's value actually is -- that is a type-table question this crate
/// does not have an answer for -- and "none of these is an object" is the one fact the syntax
/// alone already settles.
///
/// Strict only. PHP's loose `in_array` can equate an `int` with a numeric string, and reading that
/// as "not an object" either way would be the ordinary loose-comparison trap `FalseTest` above
/// already refuses.
/// `is_a($x, Foo::class)` and `is_subclass_of($x, Foo::class)` -- `instanceof` written as a call.
///
/// The same claim and the same vocabulary, reached through a name instead of an operator, which is
/// how a class is tested when the class comes from a variable or a constant rather than being
/// spelled in the source. `ClassTarget` travels straight out of the `::class` the call names, so
/// `self::class` and `static::class` resolve where they are written exactly as `instanceof self`
/// does.
///
/// Three refusals, and each is a case where the call says less than the operator:
///
/// - **A subject that may be a class name.** Both functions take `object|string`, and the third
///   parameter is what decides which -- but they *default it differently*: `is_a` refuses a string
///   unless told to accept one, and `is_subclass_of` accepts one unless told not to. So
///   `is_subclass_of($className, Foo::class)` is the ordinary way to check a `class-string` before
///   `$className::make()`, and reading it as "this value is a `Foo`" reports three correct lines on
///   one real checkout. The rule is the same for both and the default is not: a string is admitted
///   unless a literal `false` is written at the third position, and where one may arrive this proves
///   nothing about the value.
/// - **`is_subclass_of` being false.** It is false for `Foo` itself, so the negative rules nothing
///   out: `!is_subclass_of($x, Foo::class)` leaves `Foo` and everything unrelated at once.
/// - **A class named by anything but `::class`.** `is_a($x, $className)` and `is_a($x, 'Foo')` name
///   a class this layer cannot resolve to a declaration -- the first is a value and the second a
///   string nothing anchors -- and inventing one is the fallback this project refuses.
fn class_test(
    hir: &FileHir,
    name: phrost_hir::NameRefId,
    args: lspkit_arena::IdSlice<ExprId>,
    subject: Subject<'_>,
    positive: bool,
) -> Option<Proved> {
    let written = hir.name_ref(name).path.as_str().trim_start_matches('\\');
    let subclass_only = if written.eq_ignore_ascii_case("is_a") {
        false
    } else if written.eq_ignore_ascii_case("is_subclass_of") {
        true
    } else {
        return None;
    };
    let (value, class, allows_a_string) = match hir.expr_list(args) {
        // PHP's own defaults, and they are opposites: `is_a` refuses a class name, `is_subclass_of`
        // accepts one.
        [value, class] => (value, class, subclass_only),
        [value, class, allow] => (value, class, !written_false(hir, *allow)),
        _ => return None,
    };
    if allows_a_string {
        return None;
    }
    let tested = positional_argument(hir, *value)?;
    if !subject.wrote(hir, tested) {
        return None;
    }
    let of = path_of(hir, tested)?;
    let named = positional_argument(hir, *class)?;
    let ExprData::ClassConstAccess {
        class: target,
        name: Some(constant),
        ..
    } = hir.expr(named)
    else {
        return None;
    };
    if !constant.as_str().eq_ignore_ascii_case("class") {
        return None;
    }
    match (positive, subclass_only) {
        (true, _) => Some(Proved::Class { of, class: *target }),
        (false, true) => None,
        (false, false) => Some(Proved::NotClass {
            of,
            class: *target,
            exactly: false,
        }),
    }
}

/// `$x::class === Foo::class` and `get_class($x) === Foo::class`: the value's own class, compared
/// with one written out.
///
/// The side that holds is an `instanceof` -- the class is that one exactly, which is more than an
/// `instanceof` says and nothing a reader of one needs. The other side rules out `Foo` alone and
/// not what extends it, so it is a [`Proved::NotClass`] marked `exactly`.
///
/// Loose equality reads the same, because two class names are strings that are not numbers. A
/// class held in anything but `::class` -- a string, a variable -- is refused for the reason
/// [`class_test`] gives.
fn exact_class_comparison(
    hir: &FileHir,
    operator: phrost_hir::body::CompareOp,
    left: ExprId,
    right: ExprId,
    subject: Subject<'_>,
) -> Option<impl FnOnce(bool) -> Option<Proved>> {
    use phrost_hir::body::CompareOp;

    let holds_when = match operator {
        CompareOp::Identical | CompareOp::Equal => true,
        CompareOp::NotIdentical | CompareOp::NotEqual => false,
        _ => return None,
    };
    let (value, class) = [(left, right), (right, left)]
        .into_iter()
        .find_map(|(value, class)| Some((class_of_value(hir, value)?, named_class(hir, class)?)))?;
    if !subject.wrote(hir, value) {
        return None;
    }
    let of = path_of(hir, value)?;
    Some(move |positive: bool| {
        Some(match positive == holds_when {
            true => Proved::Class { of, class },
            false => Proved::NotClass {
                of,
                class,
                exactly: true,
            },
        })
    })
}

/// The value whose class an expression reads: `$x` in `$x::class` and in `get_class($x)`.
fn class_of_value(hir: &FileHir, expr: ExprId) -> Option<ExprId> {
    match hir.expr(expr) {
        ExprData::ClassConstAccess {
            class: ClassTarget::Expr(value),
            name: Some(constant),
            ..
        } if constant.as_str().eq_ignore_ascii_case("class") => Some(*value),
        ExprData::FunctionCall { name, args, .. }
            if hir
                .name_ref(*name)
                .path
                .as_str()
                .trim_start_matches('\\')
                .eq_ignore_ascii_case("get_class") =>
        {
            let [only] = hir.expr_list(*args) else {
                return None;
            };
            positional_argument(hir, *only)
        }
        _ => None,
    }
}

/// The class a `Foo::class` names, where the expression is one and the class is written rather than
/// held in a value.
fn named_class(hir: &FileHir, expr: ExprId) -> Option<ClassTarget> {
    match hir.expr(expr) {
        ExprData::ClassConstAccess {
            class,
            name: Some(constant),
            ..
        } if constant.as_str().eq_ignore_ascii_case("class")
            && !matches!(class, ClassTarget::Expr(_)) =>
        {
            Some(*class)
        }
        _ => None,
    }
}

/// `array_key_exists($key, $map)`, and its alias `key_exists`, on the side that held.
fn key_test(
    hir: &FileHir,
    name: phrost_hir::NameRefId,
    args: lspkit_arena::IdSlice<ExprId>,
    subject: Subject<'_>,
    positive: bool,
) -> Option<Proved> {
    let called = hir.name_ref(name).path.as_str().trim_start_matches('\\');
    if !positive
        || !(called.eq_ignore_ascii_case("array_key_exists")
            || called.eq_ignore_ascii_case("key_exists"))
    {
        return None;
    }
    let [key, container] = hir.expr_list(args) else {
        return None;
    };
    let key = positional_argument(hir, *key)?;
    if !subject.wrote(hir, key) {
        return None;
    }
    Some(Proved::KeyOf {
        of: path_of(hir, key)?,
        container: path_of(hir, positional_argument(hir, *container)?)?,
    })
}

/// Whether an argument is the keyword `false`, written out.
///
/// The literal and nothing else: a constant or a variable holding `false` says the same thing at
/// runtime and nothing this layer can read, and a test that is only sometimes right about which of
/// two subjects it has is worse here than one that declines.
fn written_false(hir: &FileHir, argument: ExprId) -> bool {
    positional_argument(hir, argument).is_some_and(|value| {
        matches!(
            hir.expr(value),
            ExprData::Literal {
                kind: LiteralKind::False,
                ..
            }
        )
    })
}

/// The needle and the haystack of an `in_array` call made strict by a literal `true`.
///
/// The one reading both membership claims go through, so that what counts as strict is decided
/// once: PHP's loose comparison equates `0` with `'a'` before PHP 8 and `'1'` with `1` still, so a
/// loose search proves nothing about what the needle *is*, and a flag held in a variable says
/// nothing this layer can read.
fn strict_in_array(
    hir: &FileHir,
    name: phrost_hir::NameRefId,
    args: lspkit_arena::IdSlice<ExprId>,
) -> Option<(ExprId, ExprId)> {
    if !hir
        .name_ref(name)
        .path
        .as_str()
        .trim_start_matches('\\')
        .eq_ignore_ascii_case("in_array")
    {
        return None;
    }
    let [needle, haystack, strict] = hir.expr_list(args) else {
        return None;
    };
    let strict = positional_argument(hir, *strict)?;
    if !matches!(
        hir.expr(strict),
        ExprData::Literal {
            kind: LiteralKind::True,
            ..
        }
    ) {
        return None;
    }
    Some((
        positional_argument(hir, *needle)?,
        positional_argument(hir, *haystack)?,
    ))
}

/// `in_array($x, $list, true)` that held: the needle is one of the haystack's elements.
///
/// The membership twin of [`key_test`], and read the same way -- the claim names a second value,
/// and what the needle then is depends on that value's declaration, which is the type layer's to
/// read. See [`PathRoot::ElementOf`](crate::PathRoot::ElementOf).
///
/// Only the side that held. A needle found nowhere in the list may be of any type at all, including
/// the elements' own: `in_array($user, $admins, true)` failing leaves a `User` that is simply not an
/// admin. And only strict, for the reason [`strict_in_array`] gives.
///
/// A haystack written as a literal of scalars is read here too where the needle was found, since its
/// elements are the narrower claim -- `in_array($status, ['draft', 'published'], true)` leaves a
/// string that holds something, and one of two enum cases leaves the enum. Where it was not found,
/// [`in_array_of_non_objects`] answers.
fn element_test(
    hir: &FileHir,
    name: phrost_hir::NameRefId,
    args: lspkit_arena::IdSlice<ExprId>,
    subject: Subject<'_>,
    positive: bool,
) -> Option<Proved> {
    if !positive {
        return None;
    }
    let (needle, haystack) = strict_in_array(hir, name, args)?;
    if !subject.wrote(hir, needle) {
        return None;
    }
    Some(Proved::ElementOf {
        of: path_of(hir, needle)?,
        container: path_of(hir, haystack)?,
    })
}

/// `str_starts_with($x, '$')` that held: `$x` holds at least what its needle does, since it holds
/// the needle. `str_ends_with` and `str_contains` the same.
///
/// Only a needle written as a literal, which the reader can read; and only the side that held -- a
/// string that does not start with `'$'` may be anything, the empty one included.
fn needle_test(
    hir: &FileHir,
    name: phrost_hir::NameRefId,
    args: lspkit_arena::IdSlice<ExprId>,
    subject: Subject<'_>,
    positive: bool,
) -> Option<Proved> {
    if !positive {
        return None;
    }
    let written = hir.name_ref(name).path.as_str().trim_start_matches('\\');
    if !["str_starts_with", "str_ends_with", "str_contains"]
        .iter()
        .any(|function| written.eq_ignore_ascii_case(function))
    {
        return None;
    }
    let [haystack, needle] = hir.expr_list(args) else {
        return None;
    };
    let haystack = positional_argument(hir, *haystack)?;
    let needle = positional_argument(hir, *needle)?;
    if !subject.wrote(hir, haystack)
        || !matches!(
            hir.expr(needle),
            ExprData::Literal {
                kind: LiteralKind::String,
                ..
            }
        )
    {
        return None;
    }
    Some(Proved::NotEmptyString {
        of: path_of(hir, haystack)?,
        like: Some(needle),
    })
}

/// A loose `in_array($x, ['a', 'b'])` that held, read for what it proves about a string `$x`: that
/// it holds what the haystack's strings do, since PHP 8 compares two strings loosely as strings
/// unless both spell numbers -- and no number is spelled by `''`.
///
/// Only a haystack written as string literals, which the reader judges by their text -- whether any
/// spells a number, which would let a `'0'` through. Nothing about any other kind of needle: `true`
/// is loosely equal to every non-empty string, and that alternative is left as it was.
fn loose_in_strings(
    hir: &FileHir,
    name: phrost_hir::NameRefId,
    args: lspkit_arena::IdSlice<ExprId>,
    subject: Subject<'_>,
    positive: bool,
) -> Option<Proved> {
    if !positive
        || !hir
            .name_ref(name)
            .path
            .as_str()
            .trim_start_matches('\\')
            .eq_ignore_ascii_case("in_array")
    {
        return None;
    }
    let (needle, haystack) = match hir.expr_list(args) {
        [needle, haystack] => (*needle, *haystack),
        [needle, haystack, strict] if written_false(hir, *strict) => (*needle, *haystack),
        _ => return None,
    };
    let (needle, haystack) = (
        positional_argument(hir, needle)?,
        positional_argument(hir, haystack)?,
    );
    let ExprData::ArrayLiteral { elements, .. } = hir.expr(haystack) else {
        return None;
    };
    let elements = hir.expr_list(*elements);
    if !subject.wrote(hir, needle)
        || elements.is_empty()
        || !elements.iter().all(|&element| {
            matches!(
                hir.expr(element),
                ExprData::Literal {
                    kind: LiteralKind::String,
                    ..
                }
            )
        })
    {
        return None;
    }
    Some(Proved::NotEmptyString {
        of: path_of(hir, needle)?,
        like: Some(haystack),
    })
}

fn in_array_of_non_objects(
    hir: &FileHir,
    name: phrost_hir::NameRefId,
    args: lspkit_arena::IdSlice<ExprId>,
    subject: Subject<'_>,
    positive: bool,
) -> Option<Proved> {
    // The side that held is [`element_test`]'s, which reads the literal's elements.
    if positive {
        return None;
    }
    let (needle, haystack) = strict_in_array(hir, name, args)?;
    if !subject.wrote(hir, needle) {
        return None;
    }
    let ExprData::ArrayLiteral { elements, .. } = hir.expr(haystack) else {
        return None;
    };
    let elements = hir.expr_list(*elements);
    if elements.is_empty()
        || !elements.iter().all(|&element| {
            matches!(
                hir.expr(element),
                ExprData::Literal { .. } | ExprData::ClassConstAccess { .. }
            )
        })
    {
        return None;
    }
    Some(Proved::Kind {
        of: path_of(hir, needle)?,
        kind: phrost_model::BuiltinTy::Object,
        holds: !positive,
        // A filter and never a claim of its own: the branch this reads for is the one where the
        // needle was *not* one of the haystack's values, and a value that is none of two strings is
        // not thereby an object. What it can do is remove the alternatives of a declared union that
        // are not objects, which is the whole of what this rule was written for.
        settled: false,
    })
}

/// The guard's own subject, found among the arguments of a call that is not about it directly.
///
/// `Validator::isUser($x)` and `$validator->isUser($x)` are the same shape from here: neither call
/// is *on* `$x`, so what may prove something about it is one of the arguments -- and this is the
/// position search [`Proved::AssertedArgument`] needs, shared because a static call and an
/// instance call pass their arguments the same way. Not asked of a free function: see
/// [`PathRoot::AssertedArgument`](crate::PathRoot::AssertedArgument) for why that one costs more
/// than it is worth.
///
/// The first match, not every match: `f($x, $x)` writes the same value twice, and this layer has
/// no way to tell which position a tag meant -- so picking either risks answering for the wrong
/// one on the rare call that differs by position rather than by value. In practice a value is
/// passed once.
fn asserted_argument_of(
    hir: &FileHir,
    callee: Callee,
    args: lspkit_arena::IdSlice<ExprId>,
    subject: Subject<'_>,
    positive: bool,
) -> Option<Proved> {
    for (argument, &passed) in hir.expr_list(args).iter().enumerate() {
        let (name, value) = match hir.expr(passed) {
            // A spread passes an unknown number of values at unknown positions -- `f(...$xs)` --
            // so nothing here can name which one `$x` is, even when `$x` is inside `$xs`.
            ExprData::Argument { spread: true, .. } => continue,
            ExprData::Argument { name, value, .. } => (name.clone(), *value),
            _ => (None, passed),
        };
        if !subject.wrote(hir, value) {
            continue;
        }
        return Some(Proved::AssertedArgument {
            of: path_of(hir, value)?,
            callee,
            argument,
            name,
            when_true: positive,
        });
    }
    None
}

/// What `condition` proves about `subject`, under `positive`.
fn proved_about(
    hir: &FileHir,
    condition: ExprId,
    subject: Subject<'_>,
    positive: bool,
    flags: Flags<'_>,
) -> Option<Proved> {
    let direct = proved_directly(hir, condition, subject, positive, flags);
    let chain = through_a_nullsafe_chain(hir, condition, subject, positive);
    // Composed rather than ranked, for the reason [`Proved::and_also`] gives at length: one
    // condition can prove two things about one value, and `if ($request?->isReady())` proves both
    // of them -- the receiver is there, and whatever the method's own contract says about it. The
    // arm that reads the contract matches first, so ranking would have kept the half that needs a
    // tag and dropped the half that needs nothing.
    match (direct, chain) {
        (Some(direct), Some(chain)) => Some(direct.and_also(chain, flags.at)),
        (found, None) | (None, found) => found,
    }
}

/// What a value reached through `?->` proves about the receivers it was reached through.
///
/// `if ($request?->firstInvoiceDate !== NULL)` is a guard about `$request` as much as about the
/// property, and every reader of that branch needs it to be: the body of it is written
/// `$request->firstInvoiceDate`, with a plain arrow, because the author already knows. Without this
/// that line reads as a null receiver -- a false positive on the one shape the operator exists for.
///
/// Only through `?->`. A plain `$a->b !== null` says nothing about `$a`: had it been null the
/// program would have died at the arrow rather than compared against one, so the test is not
/// evidence, it is downstream of an assumption.
///
/// Never in the other direction. `$a?->b === null` is true both where `$b` is null and where `$a`
/// is, which is exactly the ambiguity the operator introduces.
fn through_a_nullsafe_chain(
    hir: &FileHir,
    condition: ExprId,
    subject: Subject<'_>,
    positive: bool,
) -> Option<Proved> {
    if let Some(reached) = reached_non_null(hir, condition, positive) {
        return Some(Proved::NotNull {
            of: nullsafe_receiver(hir, reached, subject)?,
            refused: Refused::NullOnly,
        });
    }
    // `$order?->status === Status::Draft`: non-null when the constant is, which the reader knows.
    let (reached, constant) = reached_identical_to_a_constant(hir, condition, positive)?;
    Some(Proved::SentinelsRefusedBy {
        of: nullsafe_receiver(hir, reached, subject)?,
        other: path_of(hir, constant)?,
    })
}

/// The operand of a strict identity with a class constant, and the constant, on the branch where
/// the identity holds. See [`identity_against_a_constant`].
fn reached_identical_to_a_constant(
    hir: &FileHir,
    condition: ExprId,
    positive: bool,
) -> Option<(ExprId, ExprId)> {
    let ExprData::Comparison {
        operator,
        left,
        right,
        ..
    } = hir.expr(condition)
    else {
        return None;
    };
    [(*left, *right), (*right, *left)]
        .into_iter()
        .find_map(|(reached, constant)| {
            (identity_against_a_constant(hir, *operator, constant)? == positive)
                .then_some((reached, constant))
        })
}

/// Whether an operand of a logical chain writes to the subject.
///
/// Walked rather than matched on the operand itself, because the assignment can be anywhere inside
/// it -- `$c && ($x = f()) !== NULL` is the same rebinding written around a comparison.
fn rebinds(hir: &FileHir, operand: ExprId, subject: Subject<'_>) -> bool {
    let mut found = false;
    crate::walk::visit_expression(hir, operand, &mut |hir, expr| {
        if let ExprData::Assign { target, .. } = hir.expr(expr)
            && subject.is(hir, *target)
        {
            found = true;
        }
    });
    found
}

/// The expression a condition proves is not null, where it proves that about one at all.
fn reached_non_null(hir: &FileHir, condition: ExprId, positive: bool) -> Option<ExprId> {
    match hir.expr(condition) {
        // `$x !== null`, `isset($x)`, `!empty($x)` -- the negated spellings arrive with `positive`
        // already flipped, which is what the `NullTest` arm below relies on too.
        ExprData::NullTest { value, .. } if !positive => Some(*value),
        // An object is not null, whichever class it turns out to be.
        ExprData::Instanceof { value, .. } if positive => Some(*value),
        // The bare truthiness test, `if ($request?->date)`. Only these two shapes, because they are
        // the ones a `?->` can produce: anything else is an operator's result rather than a value
        // reached through a chain.
        ExprData::PropertyAccess { .. } | ExprData::MethodCall { .. } if positive => {
            Some(condition)
        }
        // `$order?->status === 'paid'`, and the else of `!==`: identical to something that cannot
        // be null, so not null either. The operand that is not the literal is the one reached.
        ExprData::Comparison {
            operator,
            left,
            right,
            ..
        } => [(*left, *right), (*right, *left)]
            .into_iter()
            .find_map(|(reached, other)| {
                (identity_against_a_non_null(hir, *operator, other)? == positive).then_some(reached)
            }),
        // `in_array($order?->status, ['paid', 'sent'], true)` that held: the needle is one of the
        // haystack's values, and none of them is null.
        ExprData::FunctionCall { name, args, .. } if positive => {
            let (needle, haystack) = strict_in_array(hir, *name, *args)?;
            let ExprData::ArrayLiteral { elements, .. } = hir.expr(haystack) else {
                return None;
            };
            let elements = hir.expr_list(*elements);
            (!elements.is_empty()
                && elements.iter().all(|&element| {
                    matches!(
                        hir.expr(element),
                        ExprData::Literal { kind, .. } if *kind != LiteralKind::Null
                    )
                }))
            .then_some(needle)
        }
        _ => None,
    }
}

/// The subject's path, when `expression` reaches its value through a `?->` on it.
///
/// Walked receiver-inwards through plain arrows as well as nullsafe ones, because PHP's `?->`
/// short-circuits the whole chain rather than one link: `$a?->b->c` is null when `$a` is, so a
/// non-null result proves `$a` was there. What each link must be is the *step onto the subject* --
/// that one has to be nullsafe, or the test is downstream of an assumption rather than evidence
/// for it.
fn nullsafe_receiver(hir: &FileHir, expression: ExprId, subject: Subject<'_>) -> Option<ValuePath> {
    let mut current = expression;
    loop {
        let (receiver, nullsafe) = match hir.expr(current) {
            ExprData::PropertyAccess {
                receiver, nullsafe, ..
            } => (*receiver, *nullsafe),
            ExprData::MethodCall {
                receiver, nullsafe, ..
            } => (*receiver, *nullsafe),
            _ => return None,
        };
        if nullsafe && subject.wrote(hir, receiver) {
            return path_of(hir, receiver);
        }
        current = receiver;
    }
}

/// Every `... ?? NULL` in the file, by range and in source order, found once per tree.
///
/// A superset of what [`proved_over_a_null_coalesce`] reads: the same shape, the right operand
/// naming null, anywhere in the file.
fn null_coalesces(hir: &FileHir) -> &[phrost_model::TextRange] {
    hir.derived(phrost_hir::Derived::NullCoalesces, |hir| {
        let mut found = Vec::new();
        Scope::visit_every_expression(hir, &mut |hir, id| {
            if let ExprData::Coalesce { right, range, .. } = hir.expr(id)
                && path_of(hir, *right).is_some_and(|it| it.names_null())
            {
                found.push(*range);
            }
        });
        found.sort_by_key(|range: &phrost_model::TextRange| range.start());
        found
    })
}

/// What a guard written over `$row['vin'] ?? NULL` proves about `$row`.
///
/// `??` is the third construct that reads through a null instead of warning about one: when the
/// container is null the operator hands back the right operand, so a guard that rules a null out of
/// the *result* has ruled out the container being null as well -- the value can only have come from
/// the left. `is_array($record['decision'] ?? NULL) ? $record['decision'] : NULL` is the idiom, and
/// without this the then-branch reads `$record` as still possibly null.
///
/// Only where the right operand is the null literal. `$a['k'] ?? $fallback` may be the fallback, and
/// a claim about the result then says nothing about `$a` at all.
///
/// What counts as ruling null out is answered by reading the same condition again, about the
/// coalesce itself, rather than by listing the guards that do -- that list is [`proved_about`], and
/// a second copy of it here is the copy that would learn nothing new.
fn proved_over_a_null_coalesce(
    hir: &FileHir,
    condition: ExprId,
    subject: Subject<'_>,
    positive: bool,
    flags: Flags<'_>,
) -> Option<Proved> {
    // Nothing to prove on the failing side: a guard that did not hold leaves the container null, the
    // key absent, or the value null, and those are not one answer.
    if !positive {
        return None;
    }

    // The prefilter: the file's `?? NULL`s are known once, and a condition holding none of them --
    // which is nearly every condition -- is not walked. This was asked of every sub-condition of
    // every guard at every position, and walking each was a tenth of checking a long file.
    let within = hir.expr(condition).range();
    let coalesces = null_coalesces(hir);
    let first = coalesces.partition_point(|range| range.start() < within.start());
    if !coalesces[first..]
        .iter()
        .take_while(|range| range.start() < within.end())
        .any(|range| within.contains_range(*range))
    {
        return None;
    }

    let mut found = None;
    crate::walk::visit_expression(hir, condition, &mut |hir, id| {
        if found.is_some() {
            return;
        }
        let ExprData::Coalesce { left, right, .. } = hir.expr(id) else {
            return;
        };
        if !path_of(hir, *right).is_some_and(|it| it.names_null()) {
            return;
        }
        if let Some(mine) = subject.reached_through(hir, *left)
            && let Some(result) = path_of(hir, id)
        {
            found = Some((mine, result));
        }
    });

    let (mine, result) = found?;
    rules_out_null(&proved_about(
        hir,
        condition,
        Subject::Expression(&result),
        positive,
        flags,
    )?)
    .then_some(Proved::NotNull {
        of: mine,
        refused: Refused::NullOnly,
    })
}

/// Whether a claim says the value it is about cannot be null.
///
/// The two that need saying: `Kind` rules null out only when it *names* a kind that is not null --
/// `!is_string($x)` leaves a null -- and `EitherOf` needs every operand to, because any one of them
/// may be the one that held.
fn rules_out_null(proved: &Proved) -> bool {
    match proved {
        Proved::Class { .. }
        | Proved::NotNull { .. }
        | Proved::NoneOfKinds { .. }
        | Proved::OneOfClasses { .. } => true,
        Proved::Kind { kind, holds, .. } => {
            *holds
                && *kind != phrost_model::BuiltinTy::Null
                && *kind != phrost_model::BuiltinTy::Mixed
        }
        Proved::EitherOf(parts) => !parts.is_empty() && parts.iter().all(rules_out_null),
        _ => false,
    }
}

fn proved_directly(
    hir: &FileHir,
    condition: ExprId,
    subject: Subject<'_>,
    positive: bool,
    flags: Flags<'_>,
) -> Option<Proved> {
    // The subject tested on its own: `if ($app)`, `if ($this->application)`. Checked before the
    // shapes below because a bare `$flag` is also how a boolean written down earlier is tested, and
    // the only thing telling those apart is whether the condition is the subject.
    if subject.is(hir, condition)
        && let Some(path) = path_of(hir, condition)
    {
        return Some(match positive {
            // Truthiness on both sides. It rules out every falsy value, and the type layer can name
            // two of them: `array|false` from the standard library leaves an `array`.
            true => Proved::NotNull {
                of: path,
                refused: Refused::EveryFalsy,
            },
            // `if (!$count)` on a `?int` proves nothing, and only the type layer knows that.
            false => Proved::Null {
                of: path,
                strictly: false,
            },
        });
    }

    if let Some(proved) = proved_over_a_null_coalesce(hir, condition, subject, positive, flags) {
        return Some(proved);
    }

    match hir.expr(condition) {
        ExprData::Instanceof { value, class, .. } if positive => subject
            .wrote(hir, *value)
            .then(|| path_of(hir, *value))
            .flatten()
            .map(|of| Proved::Class { of, class: *class }),
        // `$x instanceof T` being false names no class of its own -- but it says which of the
        // alternatives the value is not, and where the declaration listed them that leaves one.
        ExprData::Instanceof { value, class, .. } => subject
            .wrote(hir, *value)
            .then(|| path_of(hir, *value))
            .flatten()
            .map(|of| Proved::NotClass {
                of,
                class: *class,
                exactly: false,
            }),
        ExprData::Not { value, .. } => proved_about(hir, *value, subject, !positive, flags),
        // `is_string($x)` and its neighbours. The one shape where a guard says what kind of value it
        // is without naming a class, and the useful half is usually the negative one: after
        // `if (is_string($x)) { return; }` a `Foo|string` is a `Foo`.
        ExprData::FunctionCall { name, args, .. } => {
            if let Some(proved) = in_array_of_non_objects(hir, *name, *args, subject, positive) {
                return Some(proved);
            }
            if let Some(proved) = element_test(hir, *name, *args, subject, positive) {
                return Some(proved);
            }
            // `instanceof` spelled as a call, which is how a class held in a constant is tested.
            if let Some(proved) = class_test(hir, *name, *args, subject, positive) {
                return Some(proved);
            }
            if let Some(proved) = key_test(hir, *name, *args, subject, positive) {
                return Some(proved);
            }
            if let Some(proved) = needle_test(hir, *name, *args, subject, positive) {
                return Some(proved);
            }
            if let Some(proved) = loose_in_strings(hir, *name, *args, subject, positive) {
                return Some(proved);
            }
            // A function whose docblock says what its `true` means about an argument:
            // `@phpstan-assert-if-true WP_Error $thing` on WordPress's `is_wp_error`, and every
            // project's own `isValidFoo($x)` helper. Read only where the oracle says the name is
            // declared with such a tag -- see [`AssertsArguments`] for why the gate is not merely
            // an optimisation -- and only where `type_predicate` declines, so `is_string($x)`
            // keeps the answer that knows what the value *is* rather than what a tag claims.
            let Some(kind) = type_predicate(hir.name_ref(*name).path.as_str()) else {
                // A test that names no single kind can still rule a sentinel out, and three of
                // PHP's do -- not all of them the same one.
                if positive
                    && let Some(rules_out) = RulesOut::of(hir.name_ref(*name).path.as_str())
                    && let [only] = hir.expr_list(*args)
                    && let Some(tested) = positional_argument(hir, *only)
                    && subject.wrote(hir, tested)
                    && let Some(of) = path_of(hir, tested)
                {
                    return Some(Proved::NoneOfKinds {
                        of,
                        refused: rules_out.refused,
                        kinds: rules_out.kinds,
                    });
                }
                if !flags.asserts(hir, *name) {
                    return None;
                }
                return asserted_argument_of(
                    hir,
                    Callee::Function(*name),
                    *args,
                    subject,
                    positive,
                );
            };
            let [only] = hir.expr_list(*args) else {
                return None;
            };
            let tested = positional_argument(hir, *only)?;
            if !subject.wrote(hir, tested) {
                // `is_string($box->value)` says nothing about `$box` itself -- only about which
                // class it can be, where the property tells them apart.
                return match hir.expr(tested) {
                    ExprData::PropertyAccess {
                        receiver,
                        name: Some(property),
                        nullsafe: false,
                        ..
                    } if kind != phrost_model::BuiltinTy::Null && subject.wrote(hir, *receiver) => {
                        Some(Proved::MemberKind {
                            of: path_of(hir, *receiver)?,
                            property: property.clone(),
                            kind,
                            holds: positive,
                        })
                    }
                    _ => None,
                };
            }
            let of = path_of(hir, tested)?;
            // `is_null` is the null test spelled as a call, and the reader already has an answer
            // for that one which knows more than "not of this kind".
            Some(match kind {
                phrost_model::BuiltinTy::Null => match positive {
                    true => Proved::Null { of, strictly: true },
                    // `!is_null($x)` names null and nothing else, the way `!== null` does.
                    false => Proved::NotNull {
                        of,
                        refused: Refused::NullOnly,
                    },
                },
                kind => Proved::Kind {
                    of,
                    kind,
                    holds: positive,
                    settled: true,
                },
            })
        }
        // `if ($file = $this->load())`, which is the idiom for "get it and use it if it is there".
        //
        // The claim is about the right-hand side rather than about the name, even though the name is
        // what the subject matched: the guard sits at the assignment's own position, and asking what
        // the name held *there* would be asking from before the assignment ran.
        ExprData::Assign { target, .. } if subject.is(hir, *target) => {
            // The whole assignment and not its right-hand side, so a `/** @var T $x */` written
            // above the `if` still wins -- it is recorded on the assignment, and reading only the
            // value would step past it.
            let of = path_of(hir, condition)?;
            Some(match positive {
                // `if ($row = $this->fetch())` is a truthiness test on what was just assigned.
                true => Proved::NotNull {
                    of,
                    refused: Refused::EveryFalsy,
                },
                false => Proved::Null {
                    of,
                    strictly: false,
                },
            })
        }
        // The same node with the subject on the *other* side. `if ($handlerIsNew = null === $handler)`
        // is PHP's way of asking a question and remembering the answer, and what it proves about
        // `$handler` is whatever its value proves -- the truth of an assignment is the truth of what
        // was assigned. Symfony's `ErrorHandler::register` is written that way.
        //
        // Only where the value is itself a *test*. `if ($refund = $application->getRefundInfo())`
        // tests what the call returned, and reading it as a claim about `$application` makes the
        // call's receiver the subject of a test that is about its result -- which displaced the
        // `instanceof` beside it on one real project and lost a method the subclass declares.
        ExprData::Assign { value, .. }
            if matches!(
                hir.expr(*value),
                ExprData::NullTest { .. }
                    | ExprData::Comparison { .. }
                    | ExprData::BoolTest { .. }
                    | ExprData::Instanceof { .. }
                    | ExprData::Not { .. }
                    | ExprData::Logical { .. }
            ) =>
        {
            proved_about(hir, *value, subject, positive, flags)
        }
        ExprData::Logical {
            operator,
            left,
            right,
            ..
        } => {
            let carries = match (operator, positive) {
                (LogicalOp::And, true) | (LogicalOp::Or, false) => true,
                (LogicalOp::And, false) | (LogicalOp::Or, true) => false,
            };
            // Both operands hold, so both claims are true and taking whichever came first was wrong.
            // `if (($x = $this->find()) && $x instanceof Special)` is the idiom for "get it, and use
            // it if it is the kind I need", and taking the left one meant the assignment's "it is not
            // null" won over the `instanceof`'s "it is a `Special`" -- so every call to `Special`'s own
            // API in the branch read as undefined.
            // `$x instanceof A || $x instanceof B` holding says the value is one of them, which
            // neither operand says on its own -- so the `||` is read whole rather than dropped.
            // `!carries` is exactly `(Or, holding)` or `(And, not holding)`, so the operator alone
            // says which spelling of the disjunction this is -- and both prove the same union.
            if !carries
                && let Some(proved) = one_of_instanceof(
                    hir,
                    condition,
                    subject,
                    *operator,
                    matches!(operator, LogicalOp::And),
                )
            {
                return Some(proved);
            }

            // The same disjunction when its operands are not all `instanceof`.
            // `is_string($x) || $x instanceof Foo` proves `string|Foo`, and so does
            // `!is_string($x) && !$x instanceof Foo` past a `throw` -- the second is how an
            // early-return guard with two alternatives is written, and it was the shape a borrowed
            // corpus reported as narrowing nothing at all.
            //
            // After `one_of_instanceof`, not instead of it: several `instanceof` ask which of the
            // *declaration's* alternatives are left, which is a sharper answer than the union of
            // what each operand narrows to on its own.
            if !carries
                && let Some(proved) = either_of_claims(hir, condition, subject, *operator, flags)
            {
                return Some(proved);
            }

            // An assignment to the subject on the right rebinds it, so whatever the left proved is
            // about the value it held *before* and does not carry across:
            // `if (!empty($ids) && $ids = reset($ids))` leaves `$ids` holding what `reset` returned,
            // and composing the two claims anchored the answer on the name instead -- `not null not
            // null $ids`, with the value the assignment produced nowhere in it.
            if carries && rebinds(hir, *right, subject) {
                return proved_about(hir, *right, subject, positive, flags);
            }

            carries
                .then(|| {
                    let (left_expr, right_expr) = (*left, *right);
                    let left = proved_about(hir, left_expr, subject, positive, flags);
                    let right = proved_about(hir, right_expr, subject, positive, flags);
                    match (left, right) {
                        (Some(left), Some(right)) => Some(left.and_also(right, flags.at)),
                        (Some(found), None) => Some(through_the_base(
                            hir, right_expr, subject, positive, flags, found,
                        )),
                        (None, Some(found)) => Some(through_the_base(
                            hir, left_expr, subject, positive, flags, found,
                        )),
                        (None, None) => None,
                    }
                })
                .flatten()
        }
        // A comparison against null, `isset`, or `empty`. All four spellings arrive here as "the
        // value is null", with the negated ones already wrapped in a `Not` that flipped `positive`.
        ExprData::NullTest { value, kind, .. } if subject.wrote(hir, *value) => {
            let of = path_of(hir, *value)?;
            Some(match positive {
                true => Proved::Null {
                    of,
                    strictly: kind.strict(),
                },
                // `$x !== null` and `isset` say exactly that; `$x != null` and `!empty` say
                // truthy, which also says it -- and says more, which `refused` carries across.
                false => Proved::NotNull {
                    of,
                    refused: match kind {
                        _ if kind.strict() => Refused::NullOnly,
                        // `strlen($x) > 0` leaves a string that holds something, and `'0'` does.
                        NullTestKind::Length => Refused::TheEmptyString,
                        _ => Refused::EveryFalsy,
                    },
                },
            })
        }
        // The same test, asked about something the tested path was reached through. `isset` and
        // `empty` answer rather than warn when a null is anywhere along the way, so `isset` holding,
        // or `empty` failing, says `$row` is there as plainly as it says the element is.
        //
        // And that it is not `false`: asked of PHP 8.5, `isset($row['key'])` is false and
        // `empty($row['key'])` true for a `$row` that is `false`, `true` or `0`, so the sentinel
        // `parse_url()` and the rest of the standard library hand back is gone too. Not every falsy
        // value -- `isset('0'[0])` holds.
        //
        // Only the present side proves anything: it being false leaves `$row` null, an array
        // without the key, or an array whose value is null, and those are not one answer.
        //
        // `empty` was left out of this for a while, on a measurement whose new findings all traced
        // to other gaps the extra precision reached. Re-measured with those closed it was the other
        // way round: `if (empty($comment->comment_ID)) { return; }` is how WordPress guards what
        // `get_comment()` may not have found, and reading it took 190 findings off that corpus.
        ExprData::NullTest { value, kind, .. }
            if !positive
                && kind.reads_through_null()
                && let Some(of) = subject.reached_through(hir, *value) =>
        {
            Some(Proved::NotNull {
                of,
                refused: Refused::TheSentinels,
            })
        }
        // `isset($map[$key])` that held, for the key: the same claim `array_key_exists` makes, and
        // the commoner spelling of it.
        ExprData::NullTest {
            value,
            kind: NullTestKind::Isset,
            ..
        } if !positive => match hir.expr(*value) {
            ExprData::Index {
                base,
                index: Some(key),
                ..
            } if subject.wrote(hir, *key) => Some(Proved::KeyOf {
                of: path_of(hir, *key)?,
                container: path_of(hir, *base)?,
            }),
            _ => None,
        },
        ExprData::NullTest { .. } => None,
        // `$x === false`, and `$x !== false` arriving here as its `Not`. The negative side is the
        // useful one and is why this exists: `strpos($h, $n) !== false` is how the standard library
        // reports "found", and below that guard the value is the `int` and not the `int|false` the
        // signature promised.
        //
        // `Kind` rather than a claim of its own, because the machinery that removes an operand from
        // a union already reads it -- `BuiltinTy::False` is an operand like any other.
        ExprData::BoolTest {
            value, expected, ..
        } if subject.wrote(hir, *value) => Some(Proved::Kind {
            of: path_of(hir, *value)?,
            kind: match expected {
                true => phrost_model::BuiltinTy::True,
                false => phrost_model::BuiltinTy::False,
            },
            holds: positive,
            // `=== false` names the value outright, the way a predicate names a kind.
            settled: true,
        }),
        // `if ($count > 0)`, and the early return `if ($count <= 0) { return; }`.
        //
        // A relational comparison rules null and `false` out of exactly one of its branches, and for
        // `>` and `<=` it does so whatever it was compared against -- see
        // [`CompareOp::null_holds_whatever_it_faces`], which carries the measurement. The branch
        // neither sentinel can be in is the one that proves not-null.
        //
        // The other branch proves *nothing*, which is why this answers `None` there rather than the
        // `Null` claim the shape invites: the else of `$count > 0` holds `-5` as readily as it holds
        // null, and a `Proved::Null` would be a claim wider than the truth in the one direction
        // that silences real findings.
        ExprData::Comparison {
            operator,
            left,
            right,
            ..
        } => {
            if let Some(proved) = exact_class_comparison(hir, *operator, *left, *right, subject) {
                return proved(positive);
            }
            // Which side the subject is on decides the operator: `0 < $count` is `$count > 0`.
            let (operator, value, value_side, other_side) = match subject.wrote(hir, *left) {
                true => (*operator, *left, *left, *right),
                false if subject.wrote(hir, *right) => (operator.mirrored(), *right, *right, *left),
                false => return None,
            };
            // `$rows === []` and `$rows !== []`, which is the fourth way PHP writes "empty" and
            // the one the guard machinery could not read -- `empty()`, `count(...) === 0` and plain
            // truthiness all already settle it.
            //
            // Emphatically about *truth* rather than about null, so `strictly: false`: what the
            // comparison establishes is which side of falsy the value is on, and for an array those
            // are the same thing because `[]` is the only falsy array. The claim is wider than the
            // comparison for a value that is not an array -- `null !== []` holds and null is falsy
            // -- and the reason it is made anyway is that writing `!== []` of a nullable is already
            // a bug in the code being read, measured at zero sites on the reference project and at
            // no `possibly-null-*` finding lost.
            if let Some(empties) = empty_array_comparison(hir, operator, value_side, other_side) {
                let of = path_of(hir, value)?;
                return Some(match positive == empties {
                    true => Proved::Null {
                        of,
                        strictly: false,
                    },
                    false => Proved::NotNull {
                        of,
                        refused: Refused::EveryFalsy,
                    },
                });
            }

            // `$x !== ''`, and the else of `$x === ''`: whatever string the value is, it holds
            // something. Null is untouched by the strict spelling -- `null !== ''` holds -- and
            // ruled out with `false` by the loose one, which `null != ''` and `false != ''` both
            // fail. `'0' != ''` holds on PHP 8, so neither reaches the truthiness a bare `if ($x)`
            // proves. The branch where the comparison holds keeps what it had: the strict one is
            // read below as an identity against a value that is not null.
            if is_empty_string(hir, other_side) && !is_empty_string(hir, value_side) {
                let of = || path_of(hir, value);
                match (operator, positive) {
                    (CompareOp::NotIdentical, true) | (CompareOp::Identical, false) => {
                        return Some(Proved::NotEmptyString {
                            of: of()?,
                            like: None,
                        });
                    }
                    (CompareOp::NotEqual, true) | (CompareOp::Equal, false) => {
                        return Some(Proved::NotNull {
                            of: of()?,
                            refused: Refused::TheEmptyString,
                        });
                    }
                    _ => {}
                }
            }

            // `$x === 'draft'`, `$x === 0`, `$x !== self::EMPTY`. The other half of what
            // [`CompareOp::null_holds_whatever_it_faces`] answers for the relational operators,
            // and the half nothing had written: an identity that holds against something which
            // cannot be null proves the subject is not null, whatever else it leaves open.
            //
            // Only the branch the identity *holds* on says anything. The else of
            // `$x === 'draft'` holds null as readily as `'sent'`, which is the same reason the
            // relational arm below answers `None` on one side.
            if let Some(holds_when) = identity_against_a_non_null(hir, operator, other_side) {
                let of = path_of(hir, value)?;
                if positive == holds_when {
                    return Some(Proved::NotNull {
                        of,
                        refused: Refused::NullOnly,
                    });
                }
                // The else of `$row === [null]` says nothing about null, but it does say the value
                // is not *that* array -- which, for a value whose every other slot is already the
                // literal's, rules one value out of the slot that is not. See
                // [`PathRoot::NotIdenticalTo`](crate::PathRoot::NotIdenticalTo).
                return literal_array(hir, other_side).then_some(Proved::NotIdenticalTo {
                    of,
                    literal: other_side,
                });
            }

            // `$status === Status::Draft`. The same claim as the arm above when the constant is
            // never null, which is the declaration's to say -- so it is named and left to the
            // reader. Only the branch the identity holds on, for the same reason.
            if let Some(holds_when) = identity_against_a_constant(hir, operator, other_side) {
                return (positive == holds_when)
                    .then(|| {
                        Some(Proved::SentinelsRefusedBy {
                            of: path_of(hir, value)?,
                            other: path_of(hir, other_side)?,
                        })
                    })
                    .flatten();
            }

            let null_holds = operator.null_holds_whatever_it_faces()?;
            let of = path_of(hir, value)?;
            // Both sentinels and nothing about truth: `$n >= 0` holds for a `0`, which is falsy.
            // `false` goes with null because it compares as the least bool, so `$at > 0` on the
            // `int|false` of `strtotime()` leaves an `int`.
            (positive != null_holds).then_some(Proved::NotNull {
                of,
                refused: Refused::TheSentinels,
            })
        }
        // `false === is_string($x)`, `false === $x instanceof Y` -- a strict comparison against a
        // boolean written around something other than the subject is `!` spelled with an operator,
        // so it reads exactly like the `Not` arm above: what is inside is a guard about `subject`
        // in its own right, read at whichever polarity the literal asks for. `=== false` flips it
        // and `=== true` leaves it alone. The node exists only for the strict spelling, so there is
        // no loose `== false` here to get this wrong on.
        ExprData::BoolTest {
            value, expected, ..
        } => proved_about(hir, *value, subject, positive == *expected, flags),
        // A boolean method called on the subject. What it proves is written on the method, which is
        // in another file, so this names the call and the reader looks the tag up.
        ExprData::MethodCall {
            receiver,
            name: Some(method),
            ..
        } if subject.wrote(hir, *receiver) => Some(Proved::Asserted {
            receiver: path_of(hir, *receiver)?,
            method: method.clone(),
            when_true: positive,
        }),
        // The receiver is not the subject -- `$validator->isUser($x)` -- so what may prove
        // something is one of the *arguments* instead. Reached only when the arm above's guard
        // fails, which keeps the receiver form -- the more specific claim when both are true, as
        // for `$x->isX($x)` -- taking priority.
        ExprData::MethodCall {
            receiver,
            name: Some(method),
            args,
            ..
        } => asserted_argument_of(
            hir,
            Callee::Member {
                receiver: path_of(hir, *receiver)?,
                name: method.clone(),
            },
            *args,
            subject,
            positive,
        ),
        // A static call has no receiver the narrowed value could be, so it is always read as the
        // argument form: `Validator::isUser($x)`.
        ExprData::StaticCall {
            class,
            name: Some(method),
            args,
            ..
        } => asserted_argument_of(
            hir,
            Callee::Member {
                receiver: ValuePath::of(crate::path::class_path_root(*class)),
                name: method.clone(),
            },
            *args,
            subject,
            positive,
        ),
        // A boolean written down earlier and tested here. The recursion is where the flag's own
        // value gets read, so `$a = $x instanceof T; $b = $a; if ($b)` works for the same reason
        // `$a` alone does -- and the budget is what stops `$a = $b; $b = $a;` from being a hang.
        // Each step checks its own write against the guard, so a subject replaced anywhere between
        // the first flag and the test drops the claim.
        ExprData::Variable { name, .. } => {
            let flags = flags.spend()?;
            let written = flags.written(hir, name.as_str(), positive)?;
            if flags.rewritten(hir, subject, written.end) {
                return None;
            }
            proved_about(hir, written.value, subject, positive, flags)
        }
        _ => None,
    }
}

/// The value a path is about once every claim it is the root of is looked through.
fn innermost(mut path: &ValuePath) -> &ValuePath {
    while path.steps.is_empty()
        && let Some((of, _)) = path.root.narrows()
    {
        path = of;
    }
    path
}

/// A claim about a slot, read through what the other half of the same condition proved about the
/// value the slot is reached through.
///
/// `$row === [] || $row['name'] === ''` failing proves two things: the row is not empty, and its
/// name is not `''`. The second is about `$row['name']` and the first only about `$row`, so the
/// first was dropped -- and the second, resolved where the guard stands, read the slot through a
/// `$row` that could still be the empty shape, and so handed back the null an absent key reads as.
/// Re-anchored on the narrowed row, the slot is read the way the code after the guard reads it.
///
/// One step up only, which is the shape written: a guard about `$a` beside one about `$a['b']['c']`
/// is left as it was.
fn through_the_base(
    hir: &FileHir,
    other: ExprId,
    subject: Subject<'_>,
    positive: bool,
    flags: Flags<'_>,
    found: Proved,
) -> Proved {
    let Subject::Expression(path) = subject else {
        return found;
    };
    let Some((last, above)) = path.steps.split_last() else {
        return found;
    };
    let base = ValuePath {
        root: path.root.clone(),
        steps: above.to_vec(),
        may_be_absent: path.may_be_absent,
    };
    match proved_about(hir, other, Subject::Expression(&base), positive, flags) {
        Some(about_the_base) => found.about(ValuePath {
            root: about_the_base.root(flags.at),
            steps: vec![last.clone()],
            may_be_absent: path.may_be_absent,
        }),
        None => found,
    }
}

/// The class an argument written `X::class` names.
///
/// `Foo::class` is spelled like a constant and is a string, case-insensitively -- PHP accepts
/// `Foo::CLASS`. Read here rather than through the path layer, which turns it into the string it
/// evaluates to and loses which class was named.
fn class_constant_of(hir: &FileHir, expr: ExprId) -> Option<ClassTarget> {
    // A named argument wraps what was passed.
    let expr = match hir.expr(expr) {
        ExprData::Argument { value, .. } => *value,
        _ => expr,
    };
    match hir.expr(expr) {
        ExprData::ClassConstAccess {
            class,
            name: Some(name),
            ..
        } if name.as_str().eq_ignore_ascii_case("class") => Some(*class),
        _ => None,
    }
}

/// The class an `assert(...)` call proves for `subject`.
///
/// A function, not a construct, and special-cased for the same reason every analyser special-cases
/// it: `assert($x instanceof T)` is written precisely to tell a reader -- and a tool -- what the value
/// is. Only the unqualified global name, because that is the one PHP resolves it to.
fn asserted_by_call(
    hir: &FileHir,
    statement: StmtId,
    subject: Subject<'_>,
    flags: Flags<'_>,
) -> Option<Proved> {
    let StmtData::Expr { expr, .. } = hir.stmt(statement) else {
        return None;
    };
    let ExprData::FunctionCall {
        name: called, args, ..
    } = hir.expr(*expr)
    else {
        return None;
    };
    if !hir
        .name_ref(*called)
        .path
        .as_str()
        .eq_ignore_ascii_case("assert")
    {
        return None;
    }

    hir.expr_list(*args)
        .iter()
        .find_map(|&arg| proved_about(hir, arg, subject, true, flags))
}

/// Whether a comparison is the subject against an empty array literal, and what a *true* comparison
/// then says about emptiness.
///
/// `Some(true)` for `=== []`, which holding means the value is empty; `Some(false)` for `!== []`,
/// which holding means it is not. `None` for anything else, the empty literal included on the wrong
/// side of a relational operator -- `$rows > []` is not a thing anybody writes and would not mean
/// this.
///
/// Only the *strict* spellings. `$rows == []` is a loose comparison that `0`, `''`, `null` and
/// `false` all satisfy, so it is a truthiness test rather than a claim about an array.
fn empty_array_comparison(
    hir: &FileHir,
    operator: phrost_hir::body::CompareOp,
    value_side: ExprId,
    other_side: ExprId,
) -> Option<bool> {
    use phrost_hir::body::CompareOp;

    let empties = match operator {
        CompareOp::Identical => true,
        CompareOp::NotIdentical => false,
        _ => return None,
    };
    // The subject's own side must not also be the literal: `[] === []` says nothing about anything.
    if is_empty_array(hir, value_side) || !is_empty_array(hir, other_side) {
        return None;
    }
    Some(empties)
}

/// Whether a comparison is a *strict* identity against an operand that cannot be null, and which
/// branch it then holds on.
///
/// `Some(true)` for `===` and `Some(false)` for `!==`; `None` for a loose comparison, a relational
/// one, or an operand this layer cannot say is non-null.
///
/// Loose is excluded because it is wrong rather than merely weaker: `null == 0`, `null == ''` and
/// `null == []` all hold on PHP 8.5.10, so `$x == 0` says nothing about null at all. Only the
/// operands whose spelling settles it are admitted -- a literal that is not `null`, an array, a
/// `new`, and an operator whose result the operator itself decides. A constant, a call and a
/// property are all *usually* non-null and none of them is decidable here, so they are left to a
/// layer that can ask.
fn identity_against_a_non_null(
    hir: &FileHir,
    operator: phrost_hir::body::CompareOp,
    other_side: ExprId,
) -> Option<bool> {
    use phrost_hir::body::CompareOp;

    let holds_when = match operator {
        CompareOp::Identical => true,
        CompareOp::NotIdentical => false,
        _ => return None,
    };
    let never_null = match hir.expr(other_side) {
        ExprData::Literal { kind, .. } => *kind != phrost_hir::body::LiteralKind::Null,
        ExprData::ArrayLiteral { .. } | ExprData::New { .. } | ExprData::Operator { .. } => true,
        // `Foo::class` is spelled like a constant and is a string.
        ExprData::ClassConstAccess {
            name: Some(name), ..
        } => name.as_str().eq_ignore_ascii_case("class"),
        _ => false,
    };
    never_null.then_some(holds_when)
}

/// Whether a comparison is a strict identity against a class constant, and which branch it then
/// holds on -- the half of [`identity_against_a_non_null`] this layer cannot settle.
///
/// An enum case is never null and `const NONE = null` always is, and both are spelled `X::Y`. What
/// the constant holds is read where the declarations are, through
/// [`Proved::SentinelsRefusedBy`]. A global constant is not here: `PHP_EOL` and `null` are the two
/// commonest, and neither is a guard anybody writes.
fn identity_against_a_constant(
    hir: &FileHir,
    operator: phrost_hir::body::CompareOp,
    other_side: ExprId,
) -> Option<bool> {
    use phrost_hir::body::CompareOp;

    let holds_when = match operator {
        CompareOp::Identical => true,
        CompareOp::NotIdentical => false,
        _ => return None,
    };
    match hir.expr(other_side) {
        ExprData::ClassConstAccess {
            name: Some(name), ..
        } if !name.as_str().eq_ignore_ascii_case("class") => Some(holds_when),
        _ => None,
    }
}

/// Whether an expression is an array literal every entry of which is a scalar literal, under a key
/// the literal itself settles: `[null]`, `['a', null]`, `['id' => 0]`.
///
/// The only array an identity can be read against slot by slot. A variable, a call or a constant in
/// an entry is a value this layer cannot compare, a computed key names a slot it cannot identify,
/// and a spread brings slots from somewhere else. Bounded at eight entries, which is past any tuple
/// written by hand -- a comparison against a longer literal is a table, not a guard.
fn literal_array(hir: &FileHir, expr: ExprId) -> bool {
    let ExprData::ArrayLiteral { elements, .. } = hir.expr(expr) else {
        return false;
    };
    let elements = hir.expr_list(*elements);
    !elements.is_empty()
        && elements.len() <= 8
        && elements.iter().all(|&element| {
            let value = match hir.expr(element) {
                ExprData::ArrayElement {
                    kind: phrost_hir::body::ArrayElementKind::LiteralKey { .. },
                    value,
                    ..
                } => *value,
                ExprData::ArrayElement { .. } => return false,
                _ => element,
            };
            matches!(hir.expr(value), ExprData::Literal { .. })
        })
}

/// Whether an expression is the literal `''`, in either quoting.
///
/// Read off the literal's length, since a body holds no text: two characters are the two quotes
/// and nothing between them. No other string literal is that short -- the magic constants that are
/// strings are spelled with nine.
fn is_empty_string(hir: &FileHir, expr: ExprId) -> bool {
    matches!(
        hir.expr(expr),
        ExprData::Literal { kind: LiteralKind::String, range } if range.len() == 2.into()
    )
}

fn is_empty_array(hir: &FileHir, expr: ExprId) -> bool {
    matches!(
        hir.expr(expr),
        ExprData::ArrayLiteral { elements, .. } if hir.expr_list(*elements).is_empty()
    )
}
