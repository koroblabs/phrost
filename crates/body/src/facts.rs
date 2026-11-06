//! What a body's tree fixes about writes and guards, derived once per file revision.
//!
//! [`crate::reach`] says what it means for a write to reach a position and how surely, and the
//! guard visitors in [`crate::walk`] say which conditions hold at one. Both used to be answered by
//! walking every expression of the enclosing body for every position asked about -- and a
//! diagnostics pass asks about every position in the file, so a body was walked once per use of
//! every name in it. Measured on the generated 1 500-line file that is the worst case, those walks
//! were half of what a publish cost.
//!
//! The rules have two halves, and only one of them is about the position. Which writes a body
//! holds, which branch each sits in, whether a branch can be left, whether a `switch` arm falls
//! through, which conditions guard which spans: all of that is the tree's. What the position
//! decides is whether it is inside a given span, which is a range test. So the tree's half is
//! derived here once, as a list of write sites each carrying its chain of enclosing branches, and a
//! list of guards each carrying the span it holds over; a lookup is then a few range tests per
//! candidate instead of a walk.
//!
//! The rules themselves are stated in `reach.rs` and `walk.rs`, whose walks are kept as the oracle
//! the tests compare this against over every write and every position of every fixture. A rule
//! changed there has to be changed here, and the test is what says so.

use lspkit_arena::IdSlice;
use phrost_hir::{
    BodyId, Derived, ExprId, FileHir, MemberKind, ParamId, StmtId,
    body::{ExprData, LiteralKind, LogicalOp, StmtData, UnmodelledKind},
};
use phrost_model::{Name, TextRange, TextSize};

use crate::{
    reach::{Reach, handles},
    walk::{NeverReturns, any_call_leaves, branch_range, leaves, visit_expression},
};

/// Everything [`BodyFacts`] knows about one body.
#[derive(Default, Debug)]
struct OneBody {
    /// Every assignment, in the order the reach walk visits them.
    writes: Vec<WriteSite>,
    /// Every target of an `unset(...)`, in the same order. Apart from `writes` because only a reader
    /// of an array's keys asks about them: to every other reader of a write, `unset($row['tmp'])`
    /// rebinding `$row` would drop a guard the statement did nothing to.
    unsets: Vec<WriteSite>,
    /// Indices into `writes`, ordered by where each write starts -- see [`BodyFacts::writes_within`].
    writes_by_start: Vec<u32>,
    /// Every variable name written inside a `foreach` target of this body, sorted -- the prefilter
    /// in front of the walks that find a loop binding a name. See [`BodyFacts::foreach_may_bind`].
    foreach_names: Vec<Box<str>>,
    /// Every variable a `foreach` of this body iterates *by reference*, sorted -- the prefilter in
    /// front of the walk that finds what such a loop wrote back. See
    /// [`BodyFacts::iterated_by_reference`].
    by_reference_subjects: Vec<Box<str>>,
    /// Every statement of the body in the order the statement walk meets them, and their spans
    /// nested -- see [`BodyFacts::innermost_statement`].
    statements: Vec<StmtId>,
    statement_spans: crate::range_nest::RangeNest,
    /// Every `/** @var T $x */` statement, by where it starts: the name, the start, the type.
    annotations: Vec<(Name, TextSize, phrost_hir::TypeRefId)>,
    /// Indices into `writes` in the body's own order, per variable a write is rooted at, and the
    /// writes rooted at none -- see [`BodyFacts::writes_that_may_root_at`].
    in_order_by_root: std::collections::HashMap<Box<str>, Vec<u32>>,
    rootless: Vec<u32>,
    /// The same, per variable a write is rooted at -- see [`BodyFacts::writes_rooted_at`].
    writes_by_root: std::collections::HashMap<Box<str>, Vec<u32>>,
    /// Every `if`, `while` and `for` condition with the span it guards, in the order the guard walk
    /// emits them: an outer guard before what it encloses.
    if_guards: Vec<Guard>,
    /// The spans `if_guards` cover, nested -- see [`BodyFacts::if_guards_at`].
    if_guard_spans: crate::range_nest::RangeNest,
    /// Every `&&`, `||` and ternary condition with the operand it guards, in expression order.
    operand_guards: Vec<Guard>,
    /// The spans `operand_guards` cover, nested.
    operand_guard_spans: crate::range_nest::RangeNest,
    /// Names a closure in this body captured *by reference*.
    ///
    /// `$found = null; $rows->each(function () use (&$found) { $found = $row; }); $found->id;` is
    /// the shape, and the write is not in this body at all -- it is in the closure's, and it runs
    /// whenever the callee decides. So what the name holds here is not what the visible assignments
    /// say: reading them alone answered *definitely null* and reported the line that uses it.
    captured_by_reference: Vec<Name>,
    /// Where every `break` and `continue` of the body starts, in order -- see
    /// [`BodyFacts::overwrites`].
    jumps: Vec<TextSize>,
}

/// One assignment and where it sits.
#[derive(Debug)]
pub(crate) struct WriteSite {
    pub(crate) expr: ExprId,
    pub(crate) range: TextRange,
    /// The variable at the bottom of the target -- `$rows` for `$rows['a']['b'] = 1` and `$this`
    /// for `$this->count = 0` -- or none, for a target with no variable under it.
    pub(crate) root: Option<Name>,
    /// The branches enclosing the write, outermost first.
    chain: Box<[usize]>,
}

impl WriteSite {
    /// Whether this may write through the named variable: it does, or its target has no variable
    /// under it that this could tell -- a static property, a target too odd to read -- in which case
    /// the reader's own precise test decides, as it did for every write before there was a filter.
    pub(crate) fn rooted_at(&self, name: &str) -> bool {
        self.root.as_ref().is_none_or(|root| root.as_str() == name)
    }
}

/// A condition that holds over a span of the body, and with which polarity.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Guard {
    pub(crate) condition: ExprId,
    pub(crate) over: TextRange,
    pub(crate) positive: bool,
}

/// One branch a write can sit in, and what a position elsewhere means for the write.
///
/// Each variant is one arm of the `match` in `reach::reaching_statements` or `record_branches`,
/// with the offset-free half of its rule kept here and the range tests done at lookup.
#[derive(Debug)]
enum Node {
    /// A branch of a construct: an `if` arm, a loop body, a `catch`, a `match` arm, a ternary arm,
    /// the right operand of `??` or `&&`.
    Branch {
        range: TextRange,
        construct: TextRange,
        /// Whether taking this branch rules the others out -- see `reach::Sibling`.
        exclusive: bool,
        /// Whether control cannot come back out of it, counting only what leaves the *function*.
        leaves: Leaving,
        /// The same, counting a `break` or `continue` as leaving too.
        ///
        /// Which of the two answers depends on where the position is, which is the half the tree
        /// cannot settle: a `break` written in a branch leaves past every position still inside the
        /// loop it breaks out of, and reaches every position after it. So both are derived once and
        /// [`BodyFacts::reach_of`] picks per write.
        escapes: Leaving,
        /// Whether the construct runs its branch more than once.
        ///
        /// A loop body, and nothing else -- a `catch` shares its shape and does not repeat. What it
        /// buys is the one direction a position walk cannot see: a write *below* a read, in the
        /// same loop body, ran on the pass before. `$lowest = NULL; foreach (...) { ... $lowest->m()
        /// ... $lowest = $grade; }` is how every find-the-best loop in PHP is written, and reading
        /// only the writes above the cursor answered that `$lowest` was certainly null.
        repeats: bool,
    },
    /// A loop header's per-pass parts -- `foreach` targets, `for` conditions and step -- which are
    /// as conditional as the body they run for, without ruling anything out.
    HeaderOf {
        body: Option<TextRange>,
        from: TextSize,
    },
    /// The body of a `try`, conditional only from one of its handlers.
    TryBody { try_stmt: StmtId, from: TextSize },
    /// An arm of a `switch`, exclusive only as far as the arms between it and the position do not
    /// fall through.
    SwitchArm {
        switch: StmtId,
        construct: TextRange,
        index: usize,
        range: Option<TextRange>,
        leaves: Leaving,
    },
}

/// Whether control can come back out of a list of statements, as far as the tree alone can tell.
///
/// The tree decides `return`, `throw` and `exit`. A bare call statement it cannot: `abort();` leaves
/// only if `abort` is declared `: never`, and that is written in whatever file declares it, which
/// this layer does not read. So the tree's half is settled here once per revision, and the one case
/// it cannot settle is kept as the question -- put to the reader's oracle at lookup, where
/// `Scope::narrowing_at` already puts the same question for a guard clause. Cheap on both sides: the
/// oracle rejects nearly every call by name before resolving anything, and the walk it is asked over
/// follows one statement at each level.
#[derive(Debug)]
enum Leaving {
    /// Every path leaves, by syntax alone.
    Always,
    /// Some path falls through, and no call written in it could change that.
    Falls,
    /// Falls through unless a call in a deciding position never returns.
    Ask {
        statements: IdSlice<StmtId>,
        break_escapes: bool,
    },
}

impl Leaving {
    fn of(hir: &FileHir, statements: IdSlice<StmtId>, break_escapes: bool) -> Self {
        if leaves(hir, statements, break_escapes, &|_, _| false) {
            Self::Always
        } else if leaves(hir, statements, break_escapes, &any_call_leaves) {
            Self::Ask {
                statements,
                break_escapes,
            }
        } else {
            Self::Falls
        }
    }

    fn holds(&self, hir: &FileHir, never: NeverReturns<'_>) -> bool {
        match self {
            Self::Always => true,
            Self::Falls => false,
            Self::Ask {
                statements,
                break_escapes,
            } => leaves(hir, *statements, *break_escapes, never),
        }
    }
}

/// What `reaching_cases` needs of each arm: where it is, and whether it stops a fall-through.
#[derive(Debug)]
struct Arm {
    range: Option<TextRange>,
    leaves_with_break: Leaving,
}

/// A method, hook, function or file scope: what `Scope::enclosing_declaration` considers.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Declaration {
    pub(crate) params: IdSlice<ParamId>,
    pub(crate) body: BodyId,
    /// The body's own range, and `None` for a declaration that has no body.
    pub(crate) range: Option<TextRange>,
    /// Everything the declaration can be reached from: its parameters' names and its body.
    ///
    /// The prefilter `Scope::enclosing_declaration` tests before its exact rule, which reads every
    /// parameter. It is asked per position resolved, of every declaration in the file.
    pub(crate) reach: Option<TextRange>,
}

#[derive(Default, Debug)]
pub(crate) struct BodyFacts {
    bodies: Vec<(BodyId, OneBody)>,
    nodes: Vec<Node>,
    switches: Vec<(StmtId, Vec<Arm>)>,
    /// Every function-like declaration, in the order the scan over classes, functions and file
    /// scope considered them -- the order decides which of two equally wide declarations wins.
    declarations: Vec<Declaration>,
    /// The declarations' reaches, nested -- see [`BodyFacts::declarations_reaching`].
    reaches: crate::range_nest::RangeNest,
    /// Every exclusive construct's arms, keyed by where the construct begins -- see
    /// [`BodyFacts::exclusive_arms`].
    arms_by_start: rustc_hash::FxHashMap<TextSize, Vec<usize>>,
}

impl BodyFacts {
    /// The facts for this file's revision, derived on the first ask.
    pub(crate) fn of(hir: &FileHir) -> &Self {
        hir.derived(Derived::BodyFacts, Self::build)
    }

    /// The declarations whose reach holds `offset`, in the order [`Self::declarations`] lists them.
    ///
    /// Asked per position resolved, and a file of seven hundred test methods made the whole list
    /// the most expensive thing a check did. The reaches nest, so every one holding the offset is
    /// an ancestor of the last to start before it: that one's chain of parents is the whole answer.
    pub(crate) fn declarations_reaching(&self, offset: TextSize) -> Vec<&Declaration> {
        match self.reaches.holding(offset) {
            Some(found) => found
                .map(|index| &self.declarations[index as usize])
                .collect(),
            None => self
                .declarations
                .iter()
                .filter(|declaration| {
                    declaration
                        .reach
                        .is_some_and(|reach| reach.contains_inclusive(offset))
                })
                .collect(),
        }
    }

    /// Builds the nesting index over the declarations' reaches.
    fn index_reaches(&mut self) {
        self.reaches = crate::range_nest::RangeNest::of(
            self.declarations
                .iter()
                .map(|declaration| declaration.reach),
        );
    }

    /// The assignments in `body` that lie inside `range`, found by position rather than by reading
    /// every one.
    ///
    /// Asked once per statement by the narrowing that walks a branch, and the body's write list is
    /// every assignment in it -- so a long procedural function read thousands of writes per statement
    /// of every branch, which was most of what checking WordPress's `class-wp-query.php` cost.
    pub(crate) fn writes_within(
        &self,
        body: BodyId,
        range: TextRange,
    ) -> impl Iterator<Item = &WriteSite> {
        let one = self.body(body);
        let (writes, order): (&[WriteSite], &[u32]) =
            one.map_or((&[], &[]), |one| (&one.writes, &one.writes_by_start));
        let first =
            order.partition_point(|&index| writes[index as usize].range.start() < range.start());
        order[first..]
            .iter()
            .map(move |&index| &writes[index as usize])
            .take_while(move |site| site.range.start() < range.end())
            .filter(move |site| range.contains_range(site.range))
    }

    /// The innermost statement of `body` holding `offset`: the last, in the statement walk's order,
    /// of those whose range holds it -- the answer a walk of every statement gives, without it.
    pub(crate) fn innermost_statement(
        &self,
        hir: &FileHir,
        body: BodyId,
        offset: TextSize,
    ) -> Option<StmtId> {
        let one = self.body(body)?;
        match one.statement_spans.holding(offset) {
            Some(holding) => holding.last().map(|index| one.statements[index as usize]),
            None => one
                .statements
                .iter()
                .rev()
                .copied()
                .find(|&id| hir.stmt(id).range().contains_inclusive(offset)),
        }
    }

    /// Whether any `foreach` in `body` could bind `name` -- no, for nearly every name asked.
    ///
    /// The loop walks are asked per read of a local and walked the body to answer; a superset
    /// written down once lets them stop before starting.
    pub(crate) fn foreach_may_bind(&self, body: BodyId, name: &str) -> bool {
        self.body(body).is_some_and(|one| {
            one.foreach_names
                .binary_search_by(|known| (**known).cmp(name))
                .is_ok()
        })
    }

    /// Whether any `foreach` in `body` iterates `name` by reference -- `foreach ($name as &$v)`.
    ///
    /// Asked per read of a local, and almost always no, so written down once rather than walked.
    pub(crate) fn iterated_by_reference(&self, body: BodyId, name: &str) -> bool {
        self.body(body).is_some_and(|one| {
            one.by_reference_subjects
                .binary_search_by(|known| (**known).cmp(name))
                .is_ok()
        })
    }

    /// The `@var` statements in `body`, in the order the statement walk meets them.
    ///
    /// Asked per read of a local -- is there an annotation above this position -- and it walked the
    /// body's statements to find out. A body writes a handful.
    pub(crate) fn annotations(&self, body: BodyId) -> &[(Name, TextSize, phrost_hir::TypeRefId)] {
        self.body(body).map_or(&[], |one| &one.annotations)
    }

    /// The writes [`WriteSite::rooted_at`] would accept for `root`, in the body's own order: those
    /// rooted at it and those rooted at nothing. `None` is every write.
    ///
    /// The readers that walk a body's writes for one name asked `rooted_at` of every write in it,
    /// per read of the name; this hands them the same sequence without the ones it would refuse.
    pub(crate) fn writes_that_may_root_at<'s>(
        &'s self,
        body: BodyId,
        root: Option<&str>,
    ) -> Box<dyn Iterator<Item = &'s WriteSite> + 's> {
        let Some(one) = self.body(body) else {
            return Box::new(std::iter::empty());
        };
        let Some(root) = root else {
            return Box::new(one.writes.iter());
        };
        let rooted = one
            .in_order_by_root
            .get(root)
            .map_or(&[][..], Vec::as_slice);
        let (mut left, mut right) = (rooted.iter().peekable(), one.rootless.iter().peekable());
        Box::new(
            std::iter::from_fn(move || match (left.peek(), right.peek()) {
                (Some(&&a), Some(&&b)) if a < b => left.next(),
                (Some(_), Some(_)) => right.next(),
                (Some(_), None) => left.next(),
                (None, Some(_)) => right.next(),
                (None, None) => None,
            })
            .map(move |&index| &one.writes[index as usize]),
        )
    }

    /// The assignments in `body` rooted at the variable `name`, in the order they start.
    ///
    /// A write with no variable under it is not here, although [`WriteSite::rooted_at`] answers yes
    /// for one: a caller asking for writes *of* a name compares the target to it anyway.
    pub(crate) fn writes_rooted_at(
        &self,
        body: BodyId,
        name: &str,
    ) -> impl DoubleEndedIterator<Item = &WriteSite> {
        let one = self.body(body);
        let writes: &[WriteSite] = one.map_or(&[], |one| &one.writes);
        one.and_then(|one| one.writes_by_root.get(name))
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(move |&index| &writes[index as usize])
    }

    /// Every slot of `name` an `unset(...)` in `body` removes, in the order the reach walk would
    /// visit them.
    pub(crate) fn unsets_rooted_at<'s>(
        &'s self,
        body: BodyId,
        name: &'s str,
    ) -> impl Iterator<Item = &'s WriteSite> + 's {
        self.body(body)
            .map_or(&[][..], |one| &one.unsets)
            .iter()
            .filter(move |site| site.root.as_ref().is_some_and(|root| root.as_str() == name))
    }

    /// Every assignment in `body`, in the order the reach walk would visit them.
    pub(crate) fn writes(&self, body: BodyId) -> &[WriteSite] {
        self.body(body).map_or(&[], |facts| &facts.writes)
    }

    /// Every `if`, `while` and `for` guard in `body` that holds at `offset`, outermost first.
    pub(crate) fn if_guards_at(
        &self,
        body: BodyId,
        offset: TextSize,
    ) -> impl Iterator<Item = Guard> {
        let facts = self.body(body);
        Self::guards_holding(
            facts.map_or(&[][..], |facts| &facts.if_guards),
            facts.map(|facts| &facts.if_guard_spans),
            offset,
        )
    }

    /// The guards whose span holds `offset`, in the order they were emitted: through the nesting
    /// index where the spans nest, and by reading them all where they do not.
    fn guards_holding<'g>(
        guards: &'g [Guard],
        spans: Option<&crate::range_nest::RangeNest>,
        offset: TextSize,
    ) -> impl Iterator<Item = Guard> + 'g {
        let nested = spans.and_then(|spans| spans.holding(offset));
        let every = nested.is_none();
        nested
            .into_iter()
            .flatten()
            .map(move |index| guards[index as usize])
            .chain(
                guards
                    .iter()
                    .copied()
                    .filter(move |guard| every && guard.over.contains_inclusive(offset)),
            )
    }

    /// Every `&&`, `||` and ternary guard in `body` whose guarded operand holds `offset`.
    pub(crate) fn operand_guards_at(
        &self,
        body: BodyId,
        offset: TextSize,
    ) -> impl Iterator<Item = Guard> {
        let facts = self.body(body);
        Self::guards_holding(
            facts.map_or(&[][..], |facts| &facts.operand_guards),
            facts.map(|facts| &facts.operand_guard_spans),
            offset,
        )
    }

    /// Whether these writes fill every arm of the construct beginning at `since`, so that control
    /// reaching past it came through one of them.
    ///
    /// The join a conditional write makes has three parts -- the writes, what the name held before
    /// the construct, and what the guard that skipped them proved -- and the third and second are
    /// about a path that sometimes does not exist. `if ($c) { $x = a(); } else { $x = b(); }` writes
    /// on both, so the value after it is one of the two and never what it was before; a lazily built
    /// property written that way read as its declared `?T` and reported the line that uses it.
    ///
    /// Only an `if` with a non-empty `else`, and the ternary that is the same shape -- both push
    /// exactly two exclusive arms of one construct, and an arm that is empty or absent pushes none,
    /// which is what makes counting them the test. A `switch` needs its `default` read and a `match`
    /// its own totality rule; neither is answered here, so both decline.
    ///
    /// A write counts for an arm only when that arm is the *last* branch it is in: one nested a
    /// level deeper is conditional again inside it, and the arm as a whole then writes nothing
    /// sure.
    ///
    /// And an arm that leaves counts as filled, because no path out of it reaches anything after
    /// the construct. `elseif (!$attr = $param->getAttributes(X::class)[0] ?? null) { continue; }
    /// else { $attr = $attr->newInstance(); }` is the shape: the `else` is the only way past, so
    /// the value there is what it wrote -- and joining the arm that left put the reflection
    /// object back beside the instance. `break` and `continue` count only where the read is in the
    /// same pass of the loop, which `in_the_loop` says; read after the loop, a `continue` is one
    /// more way the value from before gets through.
    pub(crate) fn writes_fill_every_arm(
        &self,
        hir: &FileHir,
        body: BodyId,
        writes: &[ExprId],
        since: TextSize,
        in_the_loop: bool,
    ) -> bool {
        let arms = self.exclusive_arms(since);
        if arms.len() != 2 {
            return false;
        }
        let sites = self.body(body).map_or(&[][..], |facts| &facts.writes);
        // Without the `: never` question, for the reason `proved_entering` gives: an arm ending in
        // such a call is one this does not count as leaving, which is the side that joins more.
        let never: NeverReturns<'_> = &|_, _| false;
        arms.iter().all(|&arm| {
            let Node::Branch {
                range,
                leaves,
                escapes,
                ..
            } = &self.nodes[arm]
            else {
                return false;
            };
            let leaves = match in_the_loop {
                true => escapes.holds(hir, never),
                false => leaves.holds(hir, never),
            };
            leaves
                || sites.iter().any(|site| {
                    writes.contains(&site.expr)
                        && site.chain.last().is_some_and(|last| *last == arm)
                })
                // An arm that is nothing but another construct -- an `elseif`, or a block holding one
                // `if` -- is filled when that construct is. Only a whole arm: a construct beside
                // other statements is one of several things the arm does, and the last of them
                // decides.
                || (self.is_one_construct(*range)
                    && self.writes_fill_every_arm(hir, body, writes, range.start(), in_the_loop))
        })
    }

    /// Whether some exclusive construct spans exactly this range -- an arm that is one `if`.
    fn is_one_construct(&self, range: TextRange) -> bool {
        self.exclusive_arms(range.start()).iter().any(|&arm| {
            matches!(&self.nodes[arm], Node::Branch { construct, .. } if *construct == range)
        })
    }

    /// Where the construct begins whose *condition* holds `at` -- inside it and in none of its arms.
    ///
    /// The innermost, because an `elseif` is an `if` inside the outer one's `else`, and it is the
    /// inner one whose arms decide what the write leaves behind.
    pub(crate) fn condition_of(&self, at: TextSize) -> Option<TextSize> {
        let mut found: Option<TextSize> = None;
        for node in &self.nodes {
            let Node::Branch {
                construct,
                exclusive: true,
                ..
            } = node
            else {
                continue;
            };
            if !construct.contains(at) || found.is_some_and(|start| start >= construct.start()) {
                continue;
            }
            let in_an_arm = self.nodes.iter().any(|other| {
                matches!(other, Node::Branch { construct: theirs, range, exclusive: true, .. }
                    if theirs == construct && range.contains_inclusive(at))
            });
            if !in_an_arm {
                found = Some(construct.start());
            }
        }
        found
    }

    /// The exclusive arms of the construct beginning here, which is two for an `if` with a non-empty
    /// `else` and for a full ternary, and fewer for everything this declines to answer for.
    ///
    /// A lookup, from an index built with the facts. It was a scan of every branch node in the file,
    /// and recursing into an arm's own construct with the scan in hand took `check` from 180 ms to
    /// 350-520 -- which is why [`Self::writes_fill_every_arm`] declined to until the arms were
    /// indexed by their start.
    fn exclusive_arms(&self, since: TextSize) -> &[usize] {
        self.arms_by_start.get(&since).map_or(&[], Vec::as_slice)
    }

    /// How surely `site` ran before control reached `offset`, or `None` if it cannot have.
    ///
    /// The same decision `reach::reaching_branch` and `reach::record_branches` make on the way
    /// down, made here on the way up the write's own chain: outermost first, because
    /// [`Reach::inside`] keeps the outermost branch a position is not in.
    /// Where a write *below* the position runs from, when a loop brings control back to it.
    ///
    /// The one direction a position walk cannot see. `$lowest = NULL; foreach ($grades as $grade) {
    /// if ($lowest === NULL || $grade->min() < $lowest->min()) { $lowest = $grade; } }` reads
    /// `$lowest` above the only write that gives it a value, and on every pass but the first that
    /// write has run -- so answering from the writes above the cursor alone said the name was
    /// certainly null, and the guard beside it then had nothing left to narrow.
    ///
    /// Always [`Reach::Possible`], never certain: the first pass reaches the read with whatever the
    /// name held before the loop, and that value has to stay among the answers. The position handed
    /// back is the **loop's** start, which is where the reader looks for it.
    ///
    /// A `catch` shares the branch shape and is not a loop, which is why the flag is on the node
    /// rather than read off "does not rule its siblings out".
    pub(crate) fn carried_from(
        &self,
        hir: &FileHir,
        site: &WriteSite,
        offset: TextSize,
        never: NeverReturns<'_>,
    ) -> Option<TextSize> {
        if site.range.start() <= offset {
            return None;
        }
        let mut carried = None;
        for &node in &site.chain {
            let Node::Branch {
                range,
                construct,
                repeats,
                escapes,
                ..
            } = &self.nodes[node]
            else {
                continue;
            };
            if *repeats && range.contains_inclusive(offset) {
                carried = Some(construct.start());
                continue;
            }
            // A branch the write is in that leaves the loop -- `$entry = FALSE; break;` -- has no
            // pass after it, so what it wrote never comes round. Asked with `break` counting,
            // because what matters here is exactly whether control reaches the top again.
            if carried.is_some() && escapes.holds(hir, never) {
                return None;
            }
        }
        carried
    }

    /// Where the innermost branch the write is in, and the position is not, ends.
    ///
    /// What a branch contributes to a join is what the name holds where control *leaves* it, not
    /// what the assignment put there: `if ($c) { $em = $this->find(); if (!$em) { throw ...; } }`
    /// hands the join a non-null `$em`, and reading the write alone hands it the `?T` the call
    /// declares. Symfony's `UniqueEntityValidator` is written exactly that way.
    ///
    /// Innermost, because that is the branch the guard is in. The chain is walked outwards-in, so
    /// the last one that does not hold the position is it.
    pub(crate) fn branch_end(&self, site: &WriteSite, offset: TextSize) -> Option<TextSize> {
        site.chain
            .iter()
            .rev()
            .find_map(|&node| match &self.nodes[node] {
                Node::Branch { range, .. } if !range.contains_inclusive(offset) => {
                    Some(range.end())
                }
                _ => None,
            })
    }

    /// Whether a `break` or `continue` beside `site` also skips `offset`: the innermost loop the
    /// write is in holds the read too, and no `switch` stands between the write and that loop -- a
    /// `break` inside a `case` leaves the `switch`, which may be exactly where the read is.
    pub(crate) fn same_pass(&self, site: &WriteSite, offset: TextSize) -> bool {
        for &node in site.chain.iter().rev() {
            match &self.nodes[node] {
                Node::Branch {
                    range,
                    repeats: true,
                    ..
                } => return range.contains_inclusive(offset),
                Node::SwitchArm { .. } => return false,
                _ => {}
            }
        }
        false
    }

    /// Whether the write sits in a loop body the position is outside of.
    ///
    /// What it decides is [`Scope::otherwise`]: the complement of a branch guard describes the path
    /// that skipped the write, and for a loop that path includes one where the body never ran at
    /// all. `$best = NULL; foreach (...) { if ($best === NULL) { $best = $row; } } $best->` reaches
    /// the read with the `NULL` when the list is empty, so the guard inside says nothing about it
    /// -- while the same guard read from *inside* the loop, past the write, is the whole answer.
    ///
    /// Answered off the write's own chain rather than from the position, because the chain is what
    /// [`Self::reach_of`] already walks and the question is the same one: which constructs the
    /// write is in that the reader is not.
    pub(crate) fn skipped_a_loop(&self, site: &WriteSite, offset: TextSize) -> bool {
        site.chain.iter().any(|&node| {
            matches!(
                &self.nodes[node],
                Node::Branch {
                    range,
                    repeats: true,
                    ..
                } if !range.contains_inclusive(offset)
            )
        })
    }

    /// Whether control that ran `earlier` cannot get to `offset` without running `later` after it,
    /// so that what `earlier` wrote is gone by the time the name is read there.
    ///
    /// Two writes in one branch are a sequence and not two operands of a join: `if ($multipart) {
    /// $content = explode('--', $content); $content = $content[2]; }` leaves a string whichever way
    /// the condition went. Both used to reach the join, because each on its own is only *possible*
    /// at the read, and the answer was `array|string` -- reported against every string parameter the
    /// name was then handed to, on code that has run for twenty years.
    ///
    /// Structural, as the reach rule is. `later` sits in no branch `earlier` is not also in, so
    /// leaving any branch around `earlier` the ordinary way arrives before `later`. Three things
    /// carry the earlier value past it all the same, and each refuses: a `break` or `continue`
    /// between the two, which leaves to after a loop or a `switch` or comes round to a read at the
    /// top of the next pass; a handler of a `try` both are in, when that is where the read is --
    /// [`handles`], the reach rule's own test; and a loop header, which runs once per pass.
    pub(crate) fn overwrites(
        &self,
        hir: &FileHir,
        body: BodyId,
        later: &WriteSite,
        earlier: &WriteSite,
        offset: TextSize,
    ) -> bool {
        if earlier.range.end() > later.range.start()
            || later.range.end() >= offset
            || !earlier.chain.starts_with(&later.chain)
        {
            return false;
        }
        let carried = later.chain.iter().any(|&node| match &self.nodes[node] {
            Node::Branch { .. } | Node::SwitchArm { .. } => false,
            Node::HeaderOf { .. } => true,
            Node::TryBody { try_stmt, .. } => handles(hir, *try_stmt, offset),
        });
        if carried {
            return false;
        }
        let jumps = self.body(body).map_or(&[][..], |one| one.jumps.as_slice());
        let first = jumps.partition_point(|&at| at < earlier.range.end());
        jumps.get(first).is_none_or(|&at| at >= later.range.start())
    }

    pub(crate) fn reach_of(
        &self,
        hir: &FileHir,
        site: &WriteSite,
        offset: TextSize,
        never: NeverReturns<'_>,
    ) -> Option<Reach> {
        let mut reach = Reach::Certain;
        // Whether the position sits inside a loop the write is also in, which decides what a `break`
        // written deeper means: it leaves past every position still in that loop, and reaches every
        // position after it. Set while walking outwards-in, so by the time an inner branch is judged
        // the loops around it have been seen.
        let mut inside_a_loop = false;
        for &node in &site.chain {
            match &self.nodes[node] {
                Node::Branch {
                    range,
                    construct,
                    exclusive,
                    leaves,
                    escapes,
                    repeats,
                } => {
                    if range.contains_inclusive(offset) {
                        inside_a_loop |= *repeats;
                        continue;
                    }
                    let leaving = match inside_a_loop {
                        true => escapes,
                        false => leaves,
                    };
                    if (*exclusive && construct.contains_inclusive(offset))
                        || leaving.holds(hir, never)
                    {
                        return None;
                    }
                    reach = reach.inside(construct.start());
                }
                Node::HeaderOf { body, from } => {
                    if !body.is_some_and(|body| body.contains_inclusive(offset)) {
                        reach = reach.inside(*from);
                    }
                }
                Node::TryBody { try_stmt, from } => {
                    if handles(hir, *try_stmt, offset) {
                        reach = reach.inside(*from);
                    }
                }
                Node::SwitchArm {
                    switch,
                    construct,
                    index,
                    range,
                    leaves,
                } => {
                    if range.is_some_and(|range| range.contains_inclusive(offset)) {
                        continue;
                    }
                    let arms = self.arms(*switch);
                    let holding = arms.iter().position(|arm| {
                        arm.range
                            .is_some_and(|range| range.contains_inclusive(offset))
                    });
                    let reached = match holding {
                        Some(holding) if *index < holding => arms[*index..holding]
                            .iter()
                            .all(|arm| !arm.leaves_with_break.holds(hir, never)),
                        Some(_) | None => true,
                    };
                    if (!reached && construct.contains_inclusive(offset))
                        || leaves.holds(hir, never)
                    {
                        return None;
                    }
                    reach = reach.inside(construct.start());
                }
            }
        }
        Some(reach)
    }

    fn body(&self, body: BodyId) -> Option<&OneBody> {
        self.bodies
            .iter()
            .find(|(id, _)| *id == body)
            .map(|(_, facts)| facts)
    }

    fn arms(&self, switch: StmtId) -> &[Arm] {
        self.switches
            .iter()
            .find(|(id, _)| *id == switch)
            .map_or(&[], |(_, arms)| arms)
    }

    // --- building ---

    fn build(hir: &FileHir) -> Self {
        let mut facts = Self::default();
        // A declaration with no body is a scope all the same: an `interface`'s method and an
        // `abstract` one bind parameters, and leaving them out left document highlight, rename,
        // references and goto-type-definition dead on the one word a person reads an API by. What
        // they get is the file's empty body, so every reader that walks a scope's statements finds
        // an empty list rather than learning what a missing body means -- and a `None` range, so
        // the only way to reach one is by standing on a parameter's name.
        let mut declare = |params, body: Option<BodyId>| {
            let range = body.map(|body| hir.body(body).range);
            let reach = hir
                .param_list(params)
                .iter()
                .map(|&id| hir.param(id).name_range)
                .chain(range)
                .reduce(|one, other| one.cover(other));
            facts.declarations.push(Declaration {
                params,
                body: body.unwrap_or_else(|| hir.empty_body()),
                range,
                reach,
            });
        };
        for (_, class) in hir.classes() {
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                if member.kind == MemberKind::Method {
                    declare(member.params, member.body);
                }
                for &id in hir.hook_list(member.hooks) {
                    let hook = hir.hook(id);
                    declare(hook.params, hook.body);
                }
            }
        }
        for (_, function) in hir.functions() {
            declare(function.params, function.body);
        }
        declare(IdSlice::EMPTY, hir.root_body());
        facts.index_reaches();
        for (id, body) in hir.bodies() {
            let mut one = OneBody::default();
            let mut chain = Vec::new();
            facts.statements(hir, &mut one, body.statements, &mut chain);
            Self::if_guards(hir, &mut one, body.statements);
            Self::operand_guards(hir, &mut one, body.statements);
            Self::captured_by_reference(hir, &mut one, body.statements);
            crate::walk::visit_every_statement(hir, body.statements, &mut |hir, id| {
                one.statements.push(id);
                if let StmtData::Foreach {
                    subject,
                    key,
                    value,
                    ..
                } = hir.stmt(id)
                {
                    if let (ExprData::Variable { name, .. }, ExprData::Reference { .. }) =
                        (hir.expr(*subject), hir.expr(*value))
                    {
                        one.by_reference_subjects.push(name.as_str().into());
                    }
                    for target in [*key, Some(*value)].into_iter().flatten() {
                        visit_expression(hir, target, &mut |hir, expr| {
                            if let ExprData::Variable { name, .. } = hir.expr(expr) {
                                one.foreach_names.push(name.as_str().into());
                            }
                        });
                    }
                }
                if let StmtData::Annotation {
                    name, ty, range, ..
                } = hir.stmt(id)
                {
                    one.annotations.push((name.clone(), range.start(), *ty));
                }
                if let StmtData::Leave { range, .. } = hir.stmt(id) {
                    one.jumps.push(range.start());
                }
            });
            one.jumps.sort_unstable();
            one.if_guard_spans = crate::range_nest::RangeNest::of(
                one.if_guards.iter().map(|guard| Some(guard.over)),
            );
            one.operand_guard_spans = crate::range_nest::RangeNest::of(
                one.operand_guards.iter().map(|guard| Some(guard.over)),
            );
            one.statement_spans = crate::range_nest::RangeNest::of(
                one.statements.iter().map(|&id| Some(hir.stmt(id).range())),
            );
            one.foreach_names.sort_unstable();
            one.foreach_names.dedup();
            one.by_reference_subjects.sort_unstable();
            one.by_reference_subjects.dedup();
            one.writes_by_start = (0..one.writes.len())
                .map(|index| u32::try_from(index).expect("fewer than 4G writes in one body"))
                .collect();
            one.writes_by_start
                .sort_by_key(|&index| one.writes[index as usize].range.start());
            for (index, site) in one.writes.iter().enumerate() {
                let index = u32::try_from(index).expect("fewer than 4G writes in one body");
                match &site.root {
                    Some(root) => one
                        .in_order_by_root
                        .entry(root.as_str().into())
                        .or_default()
                        .push(index),
                    None => one.rootless.push(index),
                }
            }
            for &index in &one.writes_by_start {
                if let Some(root) = &one.writes[index as usize].root {
                    one.writes_by_root
                        .entry(root.as_str().into())
                        .or_default()
                        .push(index);
                }
            }
            facts.bodies.push((id, one));
        }
        for (index, node) in facts.nodes.iter().enumerate() {
            if let Node::Branch {
                construct,
                exclusive: true,
                ..
            } = node
            {
                facts
                    .arms_by_start
                    .entry(construct.start())
                    .or_default()
                    .push(index);
            }
        }
        facts
    }

    /// The names a closure written in these statements captures by reference *and writes*.
    ///
    /// The write is what makes the capture interesting. `$walk = function () use (&$walk) {...}` is
    /// how a closure calls itself and only *reads* the name, so the assignment beside it is still
    /// the whole answer; `$found = NULL; ... use (&$found) { $found = $row; }` writes it, and then
    /// what the visible assignments say is not what the name holds.
    ///
    /// Only a `function () use (&$x)` list: an arrow function captures by value whatever it does.
    fn captured_by_reference(hir: &FileHir, one: &mut OneBody, statements: IdSlice<StmtId>) {
        crate::walk::visit_statements(hir, statements, &mut |hir, expr| {
            let phrost_hir::body::ExprData::Closure {
                captures,
                body: inner,
                ..
            } = hir.expr(expr)
            else {
                return;
            };
            for &id in hir.expr_list(*captures) {
                let phrost_hir::body::ExprData::Reference { value, .. } = hir.expr(id) else {
                    continue;
                };
                let phrost_hir::body::ExprData::Variable { name, .. } = hir.expr(*value) else {
                    continue;
                };
                if Self::assigns(hir, hir.body(*inner).statements, name.as_str()) {
                    one.captured_by_reference.push(name.clone());
                }
            }
        });
    }

    /// Whether these statements assign the whole of `name`.
    ///
    /// A closure's own body, asked once per revision. `bound_by` rather than a bare match on the
    /// target, so `$found[] = $row` counts the way it does everywhere else -- it writes through the
    /// name and the caller sees the array it made.
    fn assigns(hir: &FileHir, statements: IdSlice<StmtId>, name: &str) -> bool {
        let mut found = false;
        crate::walk::visit_statements(hir, statements, &mut |hir, expr| {
            let phrost_hir::body::ExprData::Assign { target, .. } = hir.expr(expr) else {
                return;
            };
            if crate::binding::bound_by(hir, *target)
                .is_some_and(|bound| bound.name().as_str() == name)
            {
                found = true;
            }
        });
        found
    }

    /// Whether a closure in `body` took this name by reference, so something outside the visible
    /// writes may have written it.
    pub(crate) fn captured_by_reference_in(&self, body: BodyId, name: &str) -> bool {
        self.body(body).is_some_and(|facts| {
            facts
                .captured_by_reference
                .iter()
                .any(|captured| captured.as_str() == name)
        })
    }

    fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// One arm of the `match` in `reach::reaching_statements` per statement kind.
    fn statements(
        &mut self,
        hir: &FileHir,
        one: &mut OneBody,
        statements: IdSlice<StmtId>,
        chain: &mut Vec<usize>,
    ) {
        for &id in hir.stmt_list(statements) {
            let data = hir.stmt(id);
            let construct = data.range();
            let from = construct.start();
            match data {
                StmtData::Expr { expr, .. } => self.expression(hir, one, *expr, chain),
                StmtData::Return { value, .. } => {
                    if let Some(value) = value {
                        self.expression(hir, one, *value, chain);
                    }
                }
                StmtData::If {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.expression(hir, one, *condition, chain);
                    for branch in [then_branch, else_branch] {
                        let Some(range) = branch_range(hir, *branch) else {
                            continue;
                        };
                        let node = self.push(Node::Branch {
                            range,
                            construct,
                            exclusive: true,
                            leaves: Leaving::of(hir, *branch, false),
                            escapes: Leaving::of(hir, *branch, true),
                            // An `if` arm runs once; only a loop body comes round again.
                            repeats: false,
                        });
                        self.branch(hir, one, *branch, chain, node);
                    }
                }
                StmtData::Foreach {
                    subject,
                    key,
                    value,
                    body,
                    ..
                } => {
                    self.expression(hir, one, *subject, chain);
                    let header = self.push(Node::HeaderOf {
                        body: branch_range(hir, *body),
                        from,
                    });
                    chain.push(header);
                    if let Some(key) = key {
                        self.expression(hir, one, *key, chain);
                    }
                    self.expression(hir, one, *value, chain);
                    chain.pop();
                    self.shared_branch(hir, one, *body, construct, chain, true);
                }
                StmtData::While {
                    condition, body, ..
                } => {
                    self.expression(hir, one, *condition, chain);
                    self.shared_branch(hir, one, *body, construct, chain, true);
                }
                StmtData::For {
                    init,
                    conditions,
                    step,
                    body,
                    ..
                } => {
                    for &id in hir.expr_list(*init) {
                        self.expression(hir, one, id, chain);
                    }
                    let header = self.push(Node::HeaderOf {
                        body: branch_range(hir, *body),
                        from,
                    });
                    chain.push(header);
                    for list in [conditions, step] {
                        for &id in hir.expr_list(*list) {
                            self.expression(hir, one, id, chain);
                        }
                    }
                    chain.pop();
                    self.shared_branch(hir, one, *body, construct, chain, true);
                }
                StmtData::Block { body, .. } => self.statements(hir, one, *body, chain),
                StmtData::Try {
                    body,
                    catches,
                    finally,
                    ..
                } => {
                    let node = self.push(Node::TryBody { try_stmt: id, from });
                    chain.push(node);
                    self.statements(hir, one, *body, chain);
                    chain.pop();
                    for &catch in hir.catch_list(*catches) {
                        self.shared_branch(
                            hir,
                            one,
                            hir.catch(catch).body,
                            construct,
                            chain,
                            false,
                        );
                    }
                    self.statements(hir, one, *finally, chain);
                }
                StmtData::Switch { subject, cases, .. } => {
                    self.expression(hir, one, *subject, chain);
                    let list = hir.switch_case_list(*cases);
                    let arms = list
                        .iter()
                        .map(|&case| {
                            let body = hir.switch_case(case).body;
                            Arm {
                                range: branch_range(hir, body),
                                leaves_with_break: Leaving::of(hir, body, true),
                            }
                        })
                        .collect();
                    self.switches.push((id, arms));
                    for (index, &case) in list.iter().enumerate() {
                        let case = hir.switch_case(case);
                        for &id in hir.expr_list(case.conditions) {
                            self.expression(hir, one, id, chain);
                        }
                        let Some(range) = branch_range(hir, case.body) else {
                            continue;
                        };
                        let node = self.push(Node::SwitchArm {
                            switch: id,
                            construct,
                            index,
                            range: Some(range),
                            leaves: Leaving::of(hir, case.body, false),
                        });
                        self.branch(hir, one, case.body, chain, node);
                    }
                }
                StmtData::Leave { .. } | StmtData::Annotation { .. } => {}
                StmtData::Bind { bindings, .. } => {
                    for &id in hir.expr_list(*bindings) {
                        self.expression(hir, one, id, chain);
                    }
                }
                StmtData::Unmodelled { inner, kind, .. } => {
                    for &id in hir.expr_list(*inner) {
                        self.expression(hir, one, id, chain);
                    }
                    if *kind == UnmodelledKind::Unset {
                        for &id in hir.expr_list(*inner) {
                            let ExprData::Index { .. } = hir.expr(id) else {
                                continue;
                            };
                            one.unsets.push(WriteSite {
                                expr: id,
                                range: hir.expr(id).range(),
                                root: write_root(hir, id),
                                chain: chain.clone().into_boxed_slice(),
                            });
                        }
                    }
                }
            }
        }
    }

    /// A branch that does not rule its siblings out: a loop body or a `catch`.
    fn shared_branch(
        &mut self,
        hir: &FileHir,
        one: &mut OneBody,
        branch: IdSlice<StmtId>,
        construct: TextRange,
        chain: &mut Vec<usize>,
        repeats: bool,
    ) {
        let Some(range) = branch_range(hir, branch) else {
            return;
        };
        let node = self.push(Node::Branch {
            range,
            construct,
            exclusive: false,
            leaves: Leaving::of(hir, branch, false),
            escapes: Leaving::of(hir, branch, true),
            repeats,
        });
        self.branch(hir, one, branch, chain, node);
    }

    fn branch(
        &mut self,
        hir: &FileHir,
        one: &mut OneBody,
        branch: IdSlice<StmtId>,
        chain: &mut Vec<usize>,
        node: usize,
    ) {
        chain.push(node);
        self.statements(hir, one, branch, chain);
        chain.pop();
    }

    /// One statement's expression tree: its branches become nodes as `reach::record_branches`
    /// would classify them, and every assignment in it becomes a write site.
    fn expression(&mut self, hir: &FileHir, one: &mut OneBody, expr: ExprId, chain: &[usize]) {
        // Recorded first and turned into nodes after, because the visitor cannot borrow `self`.
        // A branch is recorded when its parent is visited, before anything inside it, so by the
        // time an assignment is seen every branch enclosing it is in the list.
        let mut spans: Vec<(TextRange, TextRange, bool)> = Vec::new();
        let mut sites: Vec<(ExprId, TextRange, Option<Name>, Vec<usize>)> = Vec::new();
        visit_expression(hir, expr, &mut |hir, id| {
            let whole = hir.expr(id).range();
            match hir.expr(id) {
                ExprData::Match { arms, .. } => {
                    for &arm in hir.match_arm_list(*arms) {
                        spans.push((hir.expr(hir.match_arm(arm).body).range(), whole, true));
                    }
                }
                ExprData::Conditional {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    if let Some(then_branch) = then_branch {
                        spans.push((hir.expr(*then_branch).range(), whole, true));
                    }
                    spans.push((hir.expr(*else_branch).range(), whole, true));
                }
                ExprData::Coalesce { right, .. } | ExprData::Logical { right, .. } => {
                    spans.push((hir.expr(*right).range(), whole, false));
                }
                // A compound assignment writes too, and says what it wrote when its operator does:
                // `$n .= 'x'` is a string and `$flags &= $mask` an int, whatever they held before.
                ExprData::Assign { target, range, .. }
                | ExprData::CompoundAssign {
                    target,
                    range,
                    produces: Some(_),
                    ..
                }
                | ExprData::CompoundAssign {
                    target,
                    range,
                    arithmetic: Some(_),
                    ..
                } => {
                    let inside = spans
                        .iter()
                        .enumerate()
                        .filter(|(_, (span, _, _))| span.contains_range(*range))
                        .map(|(index, _)| index)
                        .collect();
                    sites.push((id, *range, write_root(hir, *target), inside));
                }
                _ => {}
            }
        });

        let first = self.nodes.len();
        for (range, construct, exclusive) in spans {
            self.push(Node::Branch {
                range,
                construct,
                exclusive,
                leaves: Leaving::Falls,
                // One expression holds no statement, so nothing in it breaks or returns.
                escapes: Leaving::Falls,
                // A branch inside one expression -- a ternary arm, the right of `??` or `&&`.
                // None of them repeats.
                repeats: false,
            });
        }
        for (expr, range, root, inside) in sites {
            let chain = chain
                .iter()
                .copied()
                .chain(inside.into_iter().map(|index| first + index))
                .collect();
            one.writes.push(WriteSite {
                expr,
                range,
                root,
                chain,
            });
        }
    }

    /// The guards `walk::visit_guarding_ifs` emits, in its order, each with the span it holds over.
    fn if_guards(hir: &FileHir, one: &mut OneBody, statements: IdSlice<StmtId>) {
        for &id in hir.stmt_list(statements) {
            match hir.stmt(id) {
                StmtData::If {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    if let Some(over) = branch_range(hir, *then_branch) {
                        one.if_guards.push(Guard {
                            condition: *condition,
                            over,
                            positive: true,
                        });
                        Self::if_guards(hir, one, *then_branch);
                    }
                    if let Some(over) = branch_range(hir, *else_branch) {
                        one.if_guards.push(Guard {
                            condition: *condition,
                            over,
                            positive: false,
                        });
                    }
                    Self::if_guards(hir, one, *else_branch);
                }
                StmtData::While {
                    condition,
                    body,
                    tested_first: true,
                    ..
                } => {
                    if let Some(over) = branch_range(hir, *body) {
                        one.if_guards.push(Guard {
                            condition: *condition,
                            over,
                            positive: true,
                        });
                    }
                    Self::if_guards(hir, one, *body);
                }
                StmtData::For {
                    conditions, body, ..
                } => {
                    if let Some(&last) = hir.expr_list(*conditions).last()
                        && let Some(over) = branch_range(hir, *body)
                    {
                        one.if_guards.push(Guard {
                            condition: last,
                            over,
                            positive: true,
                        });
                    }
                    Self::if_guards(hir, one, *body);
                }
                StmtData::Foreach { body, .. }
                | StmtData::While { body, .. }
                | StmtData::Block { body, .. } => Self::if_guards(hir, one, *body),
                StmtData::Try {
                    body,
                    catches,
                    finally,
                    ..
                } => {
                    Self::if_guards(hir, one, *body);
                    Self::if_guards(hir, one, *finally);
                    for &catch in hir.catch_list(*catches) {
                        Self::if_guards(hir, one, hir.catch(catch).body);
                    }
                }
                StmtData::Switch { cases, .. } => {
                    for &case in hir.switch_case_list(*cases) {
                        Self::if_guards(hir, one, hir.switch_case(case).body);
                    }
                }
                StmtData::Expr { .. }
                | StmtData::Return { .. }
                | StmtData::Leave { .. }
                | StmtData::Annotation { .. }
                | StmtData::Unmodelled { .. }
                | StmtData::Bind { .. } => {}
            }
        }
    }

    /// The guards `walk::visit_guarding_operands` emits, in its order.
    fn operand_guards(hir: &FileHir, one: &mut OneBody, statements: IdSlice<StmtId>) {
        crate::walk::visit_statements(hir, statements, &mut |hir, expr| match hir.expr(expr) {
            ExprData::Logical {
                operator,
                left,
                right,
                ..
            } => one.operand_guards.push(Guard {
                condition: *left,
                over: hir.expr(*right).range(),
                positive: matches!(operator, LogicalOp::And),
            }),
            ExprData::Conditional {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(then_branch) = then_branch {
                    one.operand_guards.push(Guard {
                        condition: *condition,
                        over: hir.expr(*then_branch).range(),
                        positive: true,
                    });
                }
                one.operand_guards.push(Guard {
                    condition: *condition,
                    over: hir.expr(*else_branch).range(),
                    positive: false,
                });
            }
            // `match (true) { $x !== null => ..., default => ... }` is an `if`/`elseif` chain
            // written as an expression, and it is how a modern codebase writes one. Only where the
            // subject is the literal itself: anywhere else the arms compare *values*, and reading
            // `match ($kind) { 'a' => ... }` as a guard would prove `'a'` about `$kind`, which is
            // a claim about a scalar nothing here narrows.
            //
            // An arm holds when its own condition matched the subject and every earlier one did
            // not, which is exactly the `elseif` rule -- so both halves are emitted, and the
            // earlier ones with the opposite polarity. An arm listing two conditions proves
            // nothing of its own, because either may be the one that matched; it still proves both
            // failed for the arms below it.
            ExprData::Match { subject, arms, .. } => {
                let Some(matched) = literal_bool(hir, *subject) else {
                    return;
                };
                let list = hir.match_arm_list(*arms);
                for (index, &arm) in list.iter().enumerate() {
                    let over = hir.expr(hir.match_arm(arm).body).range();
                    // The earlier arms failed before this arm's *conditions* were evaluated, not
                    // only before its body ran -- and a guard's proof reads the subject where its
                    // condition is written. Covering only the bodies, `match (true) { is_null($k)
                    // => ..., is_array($k) => ..., default => strlen($k) }` proved the default arm
                    // from `is_array` read at its own condition, where the `is_null` above it was
                    // not in effect, and the null came back.
                    let reached = hir
                        .expr_list(hir.match_arm(arm).conditions)
                        .first()
                        .map_or(over, |&first| {
                            TextRange::new(hir.expr(first).range().start(), over.end())
                        });
                    for &earlier in &list[..index] {
                        for &condition in hir.expr_list(hir.match_arm(earlier).conditions) {
                            one.operand_guards.push(Guard {
                                condition,
                                over: reached,
                                positive: !matched,
                            });
                        }
                    }
                    if let [only] = hir.expr_list(hir.match_arm(arm).conditions) {
                        one.operand_guards.push(Guard {
                            condition: *only,
                            over,
                            positive: matched,
                        });
                    }
                }
            }
            _ => {}
        });
    }
}

/// The `true` or `false` a `match` subject was written as, and nothing else.
///
/// `match (true)` is the idiom; `match (false)` is the same shape read the other way round and
/// costs one line to support. A subject that is anything else compares values rather than testing
/// conditions.
fn literal_bool(hir: &FileHir, subject: ExprId) -> Option<bool> {
    match hir.expr(subject) {
        ExprData::Literal {
            kind: LiteralKind::True,
            ..
        } => Some(true),
        ExprData::Literal {
            kind: LiteralKind::False,
            ..
        } => Some(false),
        _ => None,
    }
}

/// The variable at the bottom of an assignment target, if there is one.
///
/// Through indexing, property access, a method call's receiver and a reference, because
/// `$rows['a'] = 1`, `$this->n = 0`, `$this->bag()->n = 0` and `&$item = ...` all write through the
/// variable underneath -- the same base `path_of` roots a slot at, which is what every reader of
/// writes filters by before asking `bound_by` about the local or `covers_slot` about the slot. A
/// target this cannot see through is `None`, and a `None` passes every filter.
fn write_root(hir: &FileHir, target: ExprId) -> Option<Name> {
    match hir.expr(target) {
        ExprData::Variable { name, .. } => Some(name.clone()),
        ExprData::Reference { value, .. } => write_root(hir, *value),
        ExprData::Index { base, .. } => write_root(hir, *base),
        ExprData::PropertyAccess { receiver, .. } | ExprData::MethodCall { receiver, .. } => {
            write_root(hir, *receiver)
        }
        _ => None,
    }
}
