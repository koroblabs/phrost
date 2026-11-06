//! A local read where nothing has given it a value *yet*.
//!
//! The positional half of [`locals_never_bound`](crate::locals_never_bound). That one asks whether
//! anything in the scope binds the name at all; this one asks whether anything binds it *above the
//! read*, which is the commoner mistake -- a line moved, a guard added around the assignment, a
//! variable renamed at one end of a function. PHP warns and evaluates it as null, so the branch
//! runs and puts a null nobody expected into whatever it feeds.
//!
//! # Only where the order is not in doubt
//!
//! A write is counted the moment it is written *anywhere above* the read, whatever branch it is in.
//! `if ($c) { $x = 1; } echo $x;` is therefore silent even though PHP warns when `$c` is false:
//! telling "the write may have been skipped" from "the write covers every path" needs an
//! exhaustiveness answer nothing here has, and an `if`/`elseif`/`else` that assigns in all three
//! arms is written constantly. So the claim made is the narrow one -- *every* binding of this name
//! is below this read -- and the wide one is left to a layer that can prove it.
//!
//! Two constructs break textual order, and both are refused rather than reasoned about:
//!
//! - A **loop** runs its own text again, so a write below a read reaches it on the next pass. The
//!   whole loop statement and not just its body, because a `while ($line !== false)` header is read
//!   after the body has assigned `$line`.
//! - A **closure body** runs when it is called, which can be anywhere, so the reads inside one are
//!   not the enclosing scope's business at all -- and the walks here stop at one for that reason.
//!   What is written *at* the closure is: a `use ($x)` capture takes the value where the closure is
//!   made, and a `use (&$x)` one may write it there.
//!
//! # Every way PHP gives a name a value
//!
//! Same failure mode as the never-bound check, and the same list, plus the ones position makes
//! matter: a missed binding is a report on correct code. `preg_match($p, $s, $matches)` fills a
//! by-reference parameter no call site declares; `global`, `static` and a `/** @var */` introduce a
//! name without writing to it; a destructuring target binds every name inside it whatever shape it
//! has; `$x ??= 1` and `$x++` write through the target they read. And `isset($x)`, `empty($x)` and
//! `$x ?? 'd'` are all legal on a name nothing has assigned -- they are how PHP *asks*.

use std::collections::{BTreeMap, BTreeSet};

use phrost_hir::{
    ExprId, FileHir,
    body::{ExprData, StmtData},
};
use phrost_model::{Name, TextRange, TextSize};

use crate::{
    binding::bound_by,
    scope::Scope,
    walk::{visit_every_statement, visit_statements},
};

/// Names PHP itself puts in every scope, beyond the superglobals.
///
/// `$argv` and `$argc` exist in a CLI script whether or not `register_argc_argv` is on, and the
/// other two are written by the engine at the point they become relevant. Nothing in the file binds
/// any of them, so without this list every use of one reads as a defect.
const ENGINE_VARIABLES: &[&str] = &["argv", "argc", "http_response_header", "php_errormsg"];

/// One local read above every binding of its name.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EarlyRead {
    pub name: Name,
    /// The read, which is where PHP would warn.
    pub range: TextRange,
    /// The first binding, which is the line that has to move up.
    pub bound_at: TextRange,
}

/// Every local in the file read above everything that could have given it a value.
///
/// `private_file_scope` is the same answer [`locals_never_bound`](crate::locals_never_bound) takes,
/// and for the same reason: a file's own top level is ordinarily the includer's scope, so its
/// variables can be assigned in a file this cannot see.
pub fn locals_read_before_bound(hir: &FileHir, private_file_scope: bool) -> Vec<EarlyRead> {
    let mut found = Vec::new();

    for scope in Scope::every_scope(hir) {
        // A shared top level cannot be ordered at all: the includer's assignments run before the
        // first line of this file.
        if scope.is_file_scope(hir) && !private_file_scope {
            continue;
        }
        let scanned = Scan::of(hir, &scope);
        // A scope holding `extract()`, `compact()`, `$$name` or an `include` can be given a name
        // this walk cannot see, so no claim about the order of any name in it is safe. Asked after
        // the walk rather than before it, because the walk is where the answer was already found --
        // the alternative is a second traversal of every scope in the project to learn that almost
        // none of them hides anything.
        if scanned.hidden {
            continue;
        }
        scanned.report(hir, &scope, &mut found);
    }

    found.sort_by_key(|entry| entry.range.start());
    found
}

/// Everything one scope does that can give a name a value, where, and every read it makes.
///
/// Gathered in two traversals and not one per question: a statement walk for what the statements
/// themselves bind, and an expression walk for everything else. That is what keeps the check off
/// the diagnostics budget -- each question asked on its own was a walk of every scope in the file,
/// and this runs on every keystroke in an open buffer.
#[derive(Default)]
struct Scan {
    /// Names bound in a way no position orders: a parameter, a `catch`, a `global`, a by-reference
    /// capture. Each of these is in effect for the whole scope, or for a region a read cannot be
    /// compared against, so a read of one is never early.
    anywhere: BTreeSet<Name>,
    /// The earliest binding of each name that a position *can* be compared against.
    earliest: BTreeMap<Name, TextRange>,
    /// Every binding, for the loop rule: a write inside a loop reaches a read above it.
    written: Vec<(Name, TextRange)>,
    /// Where every variable occurrence that is a write and not a read begins. By start offset
    /// because that identifies an occurrence exactly: two variables cannot begin at one place.
    write_sites: BTreeSet<TextSize>,
    /// Every read, in the order the expression walk reached it.
    reads: Vec<(Name, TextRange)>,
    /// Every loop statement, whose text runs more than once.
    loops: Vec<TextRange>,
    /// Whether the scope can be given a name this walk cannot see.
    hidden: bool,
}

impl Scan {
    fn of(hir: &FileHir, scope: &Scope) -> Self {
        let statements = hir.body(scope.body()).statements;
        let mut this = Self::default();

        for &id in hir.param_list(scope.params()) {
            this.anywhere.insert(hir.param(id).name.clone());
        }

        // Statements first, because what they bind has to be known before the expression walk can
        // tell a `foreach` target or a `global` from a read of the same name -- both are written as
        // ordinary variable expressions and both are reached by that walk.
        this.statements(hir, statements);

        // Parent before child, which is what lets one pass do both halves: an assignment is
        // reached before its target, so by the time the target's variable arrives it is already
        // recorded as a write rather than a read.
        visit_statements(hir, statements, &mut |hir, expr| {
            this.hidden = this.hidden || crate::occurrences::hides_a_variable(hir, expr);
            this.expression(hir, expr);
        });

        this
    }

    /// What the scope's own statements bind, and which of them run their text more than once.
    fn statements(&mut self, hir: &FileHir, statements: lspkit_arena::IdSlice<phrost_hir::StmtId>) {
        visit_every_statement(hir, statements, &mut |hir, id| match hir.stmt(id) {
            StmtData::Foreach {
                key, value, range, ..
            } => {
                self.loops.push(*range);
                // `foreach ($rows as $key => $row)` and `foreach ($rows as [$a, $b])`: the targets
                // belong to the header, which is not an assignment expression.
                for target in [*key, Some(*value)].into_iter().flatten() {
                    self.binds(hir, target);
                }
            }
            StmtData::While { range, .. } | StmtData::For { range, .. } => self.loops.push(*range),
            // `global $x;` and `static $x;` introduce the name without writing to it, and what they
            // introduce was given its value somewhere this scope cannot see -- the enclosing script
            // for the first, the previous call for the second. Both are also the one binding whose
            // own text reads as a use of the name, so each is recorded as a write site too.
            StmtData::Bind { bindings, .. } => {
                for &binding in hir.expr_list(*bindings) {
                    if let Some(bound) = bound_by(hir, binding) {
                        self.anywhere.insert(bound.name().clone());
                    }
                    self.binds(hir, binding);
                }
            }
            // `/** @var T $x */` names a variable without writing to it. Usually the statement
            // below it does the writing and the expression walk finds that, but where recovery
            // dropped that statement the annotation is the only place the name appears at all.
            StmtData::Annotation { name, .. } => {
                self.anywhere.insert(name.clone());
            }
            // A `catch` variable is bound for its clause and nowhere else, so there is no position
            // to compare a read against: one inside the clause is after it, and one outside it is a
            // different question this does not ask.
            StmtData::Try { catches, .. } => {
                for &catch in hir.catch_list(*catches) {
                    if let Some(name) = hir.catch(catch).variable.as_ref() {
                        self.anywhere.insert(name.clone());
                    }
                }
            }
            _ => {}
        });
    }

    /// What one expression binds, asks about, or reads.
    fn expression(&mut self, hir: &FileHir, expr: ExprId) {
        match hir.expr(expr) {
            ExprData::Variable { name, range } => {
                if !self.write_sites.contains(&range.start()) {
                    self.reads.push((name.clone(), *range));
                }
            }
            ExprData::Assign { target, .. } => self.binds(hir, *target),
            // Any name passed as a bare argument: whether a parameter is by reference is the
            // callee's business, and the callee may be anywhere. Positional rather than scope-wide,
            // because a call cannot have filled a variable read above it -- and the loop rule
            // covers the one shape where "above" is not "before".
            ExprData::MethodCall { .. }
            | ExprData::StaticCall { .. }
            | ExprData::FunctionCall { .. }
            | ExprData::IndirectCall { .. }
            | ExprData::New { .. } => {
                crate::binding::argument_bindings(hir, expr, &mut |name, range| {
                    self.record(name.clone(), range);
                });
            }
            // `use (&$rows)` hands the closure the outer variable to write, and the closure runs
            // when something calls it -- which can be before this line is reached again. A
            // by-value capture is an ordinary read and is not this.
            ExprData::Closure { captures, .. } => {
                for &id in hir.expr_list(*captures) {
                    if matches!(hir.expr(id), ExprData::Reference { .. })
                        && let Some(bound) = bound_by(hir, id)
                    {
                        self.anywhere.insert(bound.name().clone());
                    }
                }
            }
            // `isset($x)`, `empty($x)` and `$x ?? 'd'` are the three ways PHP asks whether a name
            // has been given a value, and all three are legal on one that has not. Asking is the
            // author saying they know it may be absent, so the name is left alone for the whole
            // scope rather than only inside the test: a read they guarded and a read below the
            // guard are the same intent, and this walk cannot tell a guarded read from a
            // reachable one.
            ExprData::NullTest { value, kind, .. } if kind.reads_through_null() => {
                self.asked_about(hir, *value);
            }
            ExprData::Coalesce { left, .. } => self.asked_about(hir, *left),
            // `$x .= 'a'` and `$x++` write through the target they read, so the name is read
            // here whatever else happens to it. This used to be recognised inside an unmodelled
            // node, by the target starting where the whole expression does; lowering models the
            // shape now, and the write site is still recorded so the walk below does not count the
            // same variable a second time.
            ExprData::CompoundAssign { target, .. } => {
                if let ExprData::Variable { name, range: at } = hir.expr(*target) {
                    self.record(name.clone(), *at);
                    self.write_sites.insert(at.start());
                }
            }
            _ => {}
        }
    }

    /// Records every name a written target brings into being, with where each is written.
    ///
    /// Through the target's own shape rather than by range, because a target holds reads as well as
    /// writes: `$rows[$i] = 1` creates `$rows` and *reads* `$i`. Every shape PHP can write to is
    /// followed -- a reference wraps its target, an index writes through its base, and a
    /// destructuring holds one target per name, `list(...)` included, which arrives unmodelled.
    fn binds(&mut self, hir: &FileHir, target: ExprId) {
        match hir.expr(target) {
            ExprData::Variable { name, range } => {
                self.record(name.clone(), *range);
                self.write_sites.insert(range.start());
            }
            ExprData::Reference { value, .. } | ExprData::Index { base: value, .. } => {
                self.binds(hir, *value);
            }
            // `static $x = 1;`, whose binding arrives as the assignment it is written as.
            ExprData::Assign { target, .. } => self.binds(hir, *target),
            // `[$a, $b] = ...`, `['k' => $v] = ...`, and `list($a, $b) = ...`, which lowering does
            // not model and hands over as the targets it holds.
            ExprData::ArrayLiteral { elements, .. } => {
                for &element in hir.expr_list(*elements) {
                    self.binds(hir, element);
                }
            }
            ExprData::ArrayElement { value, .. } => self.binds(hir, *value),
            ExprData::Unmodelled { inner, .. } => {
                for &id in hir.expr_list(*inner) {
                    self.binds(hir, id);
                }
            }
            _ => {}
        }
    }

    /// Records the name a presence test was asked about, however deep in a chain it is written.
    ///
    /// `isset($row['app']['id'])` asks about `$row`: the test is willing to find any part of the
    /// path missing, and the name is the only part this layer answers about.
    fn asked_about(&mut self, hir: &FileHir, tested: ExprId) {
        match hir.expr(tested) {
            ExprData::Variable { name, .. } => {
                self.anywhere.insert(name.clone());
            }
            ExprData::Index { base: inner, .. }
            | ExprData::PropertyAccess {
                receiver: inner, ..
            }
            | ExprData::Reference { value: inner, .. } => self.asked_about(hir, *inner),
            _ => {}
        }
    }

    fn record(&mut self, name: Name, at: TextRange) {
        self.earliest
            .entry(name.clone())
            .and_modify(|current| {
                if at.start() < current.start() {
                    *current = at;
                }
            })
            .or_insert(at);
        self.written.push((name, at));
    }

    /// The reads this scope makes above everything that could have bound them.
    ///
    /// One per name, the earliest, the same way the never-bound check reports one: a name read four
    /// times before it is assigned is one mistake, and four underlines say it is four.
    fn report(self, hir: &FileHir, scope: &Scope, found: &mut Vec<EarlyRead>) {
        let mut reads = self.reads.clone();
        // The expression walk reaches a parent before its children, which is not source order:
        // `$a = $b` hands over the assignment, then the target, then the value.
        reads.sort_by_key(|(_, range)| range.start());

        let mut reported: BTreeSet<Name> = BTreeSet::new();
        for (name, range) in reads {
            if reported.contains(&name) || self.anywhere.contains(&name) {
                continue;
            }
            // `$this` is bound by the call, a superglobal by the request, and the engine's own
            // variables by the engine.
            if name.as_str() == "this"
                || phrost_model::SUPERGLOBALS.contains(&name.as_str())
                || ENGINE_VARIABLES.contains(&name.as_str())
            {
                continue;
            }
            // A name the enclosing scope owns -- a `use ($outer)` capture, an arrow function's
            // implicit one -- was given its value out there, where this walk is not looking.
            if scope.reaches_outward(hir, name.as_str()) {
                continue;
            }
            // Nothing binds it anywhere, which is the never-bound check's finding and not this
            // one's. Two reports for one name would be two mistakes where there is one.
            let Some(&bound_at) = self.earliest.get(&name) else {
                continue;
            };
            if bound_at.start() <= range.start() {
                continue;
            }
            // A loop runs its own text again, so a binding below the read reaches it on the pass
            // after -- and one written outside the loop the read is in does not, which is why the
            // binding has to share the loop rather than merely exist in one.
            if self.rebound_by_a_loop(&name, range) {
                continue;
            }

            reported.insert(name.clone());
            found.push(EarlyRead {
                name,
                range,
                bound_at,
            });
        }
    }

    /// Whether a loop containing this read also contains a binding of the name.
    fn rebound_by_a_loop(&self, name: &Name, read: TextRange) -> bool {
        self.loops
            .iter()
            .filter(|loop_range| loop_range.contains_range(read))
            .any(|loop_range| {
                self.written
                    .iter()
                    .any(|(bound, at)| bound == name && loop_range.contains_range(*at))
            })
    }
}
