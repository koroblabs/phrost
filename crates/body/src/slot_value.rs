//! What a value reached through a path holds here: the last thing written to it, or what a guard
//! proved -- whichever the code says last.
//!
//! The counterpart of the answer a bare local already gets. `local_binding` ranks a name's
//! assignments against the guards around it and hands back one claim; everything reached through a
//! path -- `$this->request`, `$order->total`, `self::$cache` -- had only the guard half, so a
//! property assigned a non-null value on one line and read on the next still answered the `?T` its
//! declaration says, and every check downstream reported it:
//!
//! ```php
//! $this->request = $this->buildRequest($options);
//! $this->set = $this->engine()->run($this->request);   // "$request takes QuoteRequest, and this
//! ```                                                  //  may be null"
//!
//! # What a call in between could have done
//!
//! Nothing here asks, and that is the one thing this is deliberately unsound about. PHP lets any
//! method called between the write and the read assign to the same property, so a helper that nulls
//! it is a null this will not report. Deciding it needs the callee's *body*, and phrost lowers a
//! body only for an open buffer -- PHPStan can afford the question because it analyses a whole
//! project at once, and answers it by inferring which calls are pure. What is left is the claim
//! this layer can actually make: nothing *in this body* wrote the slot again.
//!
//! What is written in this body is not waved through, though. A write that may have run since
//! (`if ($c) { $this->request = NULL; }`) is a second answer nobody can choose between, and a write
//! to anything the slot is reached *through* replaces it outright -- both withdraw the claim
//! rather than ranking against it.

use phrost_hir::{ExprId, FileHir, body::ExprData};
use phrost_model::{Name, TextSize};

use crate::{
    facts::{BodyFacts, WriteSite},
    locals::MaybeWrite,
    narrowing::Subject,
    path::{PathRoot, PathStep, ValuePath, covers_slot, path_of, same_slot},
    reach::Reach,
    scope::Scope,
    walk::NeverReturns,
};

/// What a body says a value holds at a position, and where to read it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotValue {
    pub value: ValuePath,
    /// The assignment's own position, for a claim that came from one.
    ///
    /// Because `$this->next = $this->next->after()` reads its own subject: resolved where it was
    /// asked for, the receiver finds this same assignment and the walk goes round until it runs out
    /// of budget. `LocalBinding::written_at` carries the position for that reason and this is the
    /// same reason. `None` for a guard, which describes the value where the reader is standing.
    pub written_at: Option<TextSize>,
}

/// The name of every property the file assigns to, anywhere in it.
///
/// The cheap half of the question below, and the reason [`slot_value`] takes it rather than asking
/// for itself: the precise answer is a walk of the enclosing body, the walk is paid *per member
/// access*, and a name nothing in the file ever assigns cannot have a write to find. Measured on
/// the reference project, asking every access precisely cost 6 % of `phrost check`; this rules out
/// all but a handful of them with one comparison, and is computed once per request by the caller
/// that has somewhere to keep it.
///
/// Deliberately a superset: it says nothing about *which* value was assigned, only that the name
/// was, so anything it admits is still put through the walk.
pub fn assigned_properties(hir: &FileHir) -> Vec<Name> {
    let mut names: Vec<Name> = Vec::new();

    // Closures included, for the reason the assignment walk includes them: an assignment inside one
    // is an assignment, and this must admit everything the precise answer could find.
    Scope::visit_every_expression(hir, &mut |hir, expr| {
        let ExprData::Assign { target, .. } = hir.expr(expr) else {
            return;
        };
        let (ExprData::PropertyAccess {
            name: Some(name), ..
        }
        | ExprData::StaticPropertyAccess {
            name: Some(name), ..
        }) = hir.expr(*target)
        else {
            return;
        };
        if !names.contains(name) {
            names.push(name.clone());
        }
    });

    names
}

/// What `subject` holds at `offset`, when something in this body settles it.
///
/// For a path [`ValuePath::can_be_guarded`] admits, which is one with steps or a bare call. A bare
/// local is answered by `local_binding`, which ranks the same two claims against each other and
/// knows about the writes a branch may have made besides -- and a literal or a `new` has no slot at
/// all. Asked here as well as by the caller, because the two used to say it separately and only one
/// of them learned about the call.
///
/// `assigned` is [`assigned_properties`] for this file, which the caller holds because it is one
/// answer for every access in the file.
pub fn slot_value(
    hir: &FileHir,
    offset: TextSize,
    subject: &ValuePath,
    never: NeverReturns<'_>,
    asserts: crate::walk::AssertsArguments<'_>,
    assigned: &[Name],
) -> Option<SlotValue> {
    if !subject.can_be_guarded() {
        return None;
    }

    let guard = crate::narrowing::narrowed_with_position(hir, offset, subject, never, asserts);
    let written = match subject.steps.last() {
        Some(PathStep::Property(_)) => written_at(hir, offset, subject, never, asserts, assigned),
        // What a write put into an element is `element_written`'s question. Here it only says
        // whether a guard is still about the value that is there -- so it is asked only where there
        // is a guard, which keeps the walk off every other subscript.
        Some(PathStep::Element(Some(_))) if guard.is_some() => {
            last_write(hir, offset, subject, never).map(Written::Withdrawn)
        }
        _ => None,
    };

    // The later one, because each describes the value from where it is written onwards: a guard
    // inside a branch that begins by assigning the slot says nothing about what was there before,
    // and an assignment before a guard is exactly what the guard then narrows.
    match (guard, written) {
        (Some((proved, guard)), Some(written)) if proved > written.at() => Some(SlotValue {
            value: guard,
            written_at: None,
        }),
        (_, Some(Written::Value(at, value))) => Some(SlotValue {
            value,
            written_at: Some(at),
        }),
        // A write since the guard that nobody can name the result of. The guard is about a value
        // that may no longer be there, so it withdraws as well, and the declaration answers:
        // `if (isset($this->p)) { return $this->p; }` above `if ($c) { ... } else { $this->p = 'a'; }`
        // read the null the guard proved and reported returning it on every path.
        (_, Some(Written::Withdrawn(_))) => None,
        (Some((_, guard)), None) => Some(SlotValue {
            value: guard,
            written_at: None,
        }),
        (None, None) => None,
    }
}

/// What the writes before a position say about a slot.
enum Written {
    /// The last write control had to go through, or the join of the ones that may have run.
    Value(TextSize, ValuePath),
    /// A write that may have run, or one to something the slot is reached through, after which
    /// nothing here can name what the slot holds -- and so nothing proved before it still stands.
    Withdrawn(TextSize),
}

impl Written {
    fn at(&self) -> TextSize {
        match self {
            Self::Value(at, _) | Self::Withdrawn(at) => *at,
        }
    }
}

/// Where the last write that may have reached `subject`'s slot is, through it or through anything
/// it is reached through. See [`written_at`] for the innermost-scope rule.
fn last_write(
    hir: &FileHir,
    offset: TextSize,
    subject: &ValuePath,
    never: NeverReturns<'_>,
) -> Option<TextSize> {
    let scope = Scope::chain_at(hir, offset).pop()?;
    let facts = BodyFacts::of(hir);
    facts
        .writes_that_may_root_at(scope.body(), subject_root(subject))
        .filter(|site| root_may_cover(site, subject) && site.range.end() < offset)
        .filter(|site| facts.reach_of(hir, site, offset, never).is_some())
        .filter(|site| {
            let ExprData::Assign { target, .. } = hir.expr(site.expr) else {
                return false;
            };
            path_of(hir, *target).is_some_and(|written| covers_slot(&written, subject))
        })
        .map(|site| site.range.start())
        .max()
}

/// The variable a slot is reached through, which is the only one whose writes can cover it.
fn subject_root(subject: &ValuePath) -> Option<&str> {
    match &subject.root {
        PathRoot::Local(name) => Some(name.as_str()),
        PathRoot::This => Some("this"),
        _ => None,
    }
}

/// The last assignment to `subject`'s slot that control had to go through, and where it is written
/// -- or, where the last writes sit in branches, the join of what they wrote with what the slot held
/// before them, exactly as a local's search answers.
///
/// A property only, which is what an assignment can settle that a declaration cannot: an element
/// has [`crate::element_written`] for the same question, and a method return is not a slot anybody
/// writes to.
///
/// The innermost scope only. A write in the body around a closure ran before the closure was
/// *made*, not before it was called, so what it left in a property is not something the closure's
/// text can be read as proving.
fn written_at(
    hir: &FileHir,
    offset: TextSize,
    subject: &ValuePath,
    never: NeverReturns<'_>,
    asserts: crate::walk::AssertsArguments<'_>,
    assigned: &[Name],
) -> Option<Written> {
    let PathStep::Property(name) = subject.steps.last()? else {
        return None;
    };
    if !assigned.contains(name) {
        return None;
    }

    let scope = Scope::chain_at(hir, offset).pop()?;
    let mut settled: Option<(TextSize, ExprId)> = None;
    let mut maybe: Vec<MaybeWrite<'_>> = Vec::new();
    let mut withdrawn: Option<TextSize> = None;

    // Only the writes through the slot's own variable can cover it, so the body's other writes are
    // not even looked at; a subject rooted elsewhere sees them all, as the walk did.
    let facts = BodyFacts::of(hir);
    for site in facts.writes_that_may_root_at(scope.body(), subject_root(subject)) {
        // Inclusive at the end for the reason `assignment_before` gives: a write the cursor sits
        // at the end of is still being written.
        if !root_may_cover(site, subject) || site.range.end() >= offset {
            continue;
        }
        let Some(reach) = facts.reach_of(hir, site, offset, never) else {
            continue;
        };
        let (expr, range) = (site.expr, site.range);
        {
            let ExprData::Assign { target, .. } = hir.expr(expr) else {
                continue;
            };
            let Some(written) = path_of(hir, *target) else {
                continue;
            };
            if !covers_slot(&written, subject) {
                continue;
            }
            let at = range.start();
            // The assignment and not its right-hand side, for the reason a local's search gives: a
            // `/** @var T */` above it is recorded on the assignment, and reading only the value
            // would step past it.
            match (reach, same_slot(&written, subject)) {
                (Reach::Certain, true) => {
                    if settled.is_none_or(|(had, _)| had < at) {
                        settled = Some((at, expr));
                    }
                }
                // A write that may have run: one operand of a join, the way a local's is.
                (Reach::Possible(since), true) => maybe.push(MaybeWrite {
                    at,
                    expr,
                    site,
                    since,
                    // A property write takes the whole value; only a local can be destructured.
                    slot: Vec::new(),
                    // A slot's writes are read from the statements before the position only, so a
                    // loop can never carry one round to here.
                    carried: false,
                    repeated: facts.skipped_a_loop(site, offset),
                    ends: facts.branch_end(site, offset),
                    same_pass: facts.same_pass(site, offset),
                }),
                // A write to a value this slot is reached through. From here on nobody can name
                // what the slot holds, whether the write ran or only may have.
                (_, false) => {
                    if withdrawn.is_none_or(|had| had < at) {
                        withdrawn = Some(at);
                    }
                }
            }
        }
    }

    // A write control had to go through erases anything conditional above it: whichever branch
    // ran, this one ran after.
    if let Some((at, _)) = settled {
        maybe.retain(|write| write.at > at);
    }
    let Some(latest) = settled
        .map(|(at, _)| at)
        .into_iter()
        .chain(maybe.iter().map(|write| write.at))
        .max()
    else {
        return withdrawn.map(Written::Withdrawn);
    };
    if let Some(withdrawn) = withdrawn
        && withdrawn > latest
    {
        return Some(Written::Withdrawn(withdrawn));
    }
    if !maybe.is_empty() {
        // Joined whatever the branch guard proved about the skipped path, because what the slot held
        // before the construct is one of the operands -- read where the construct begins, with any
        // guard above it in effect. This used to withdraw where the branch guard said nothing,
        // because a union of `Foo` with `?Foo` collapsed to the class and the null went with it;
        // the join keeps a nullable operand now, and withdrawing threw away the guard above:
        // `if ($this->pending === null) { throw ...; }` then `if ($this->pending < 0) {
        // $this->pending = 0; }` read the declared `?int`, and every return of it was reported.
        let subject_of = Subject::Expression(subject);
        return Some(
            match scope.maybe_written(hir, &maybe, offset, subject.clone(), subject_of, asserts) {
                Some((at, value)) => Written::Value(at, value),
                None => Written::Withdrawn(latest),
            },
        );
    }
    let (at, expr) = settled?;
    Some(Written::Value(at, path_of(hir, expr)?))
}

/// Whether a write could cover `subject`, judged by the variable under its target alone.
///
/// A slot is reached through a local or through `$this`, and only a write through the same
/// variable can cover it; the precise test is `covers_slot`, and this keeps it from being asked
/// about every write in the body. A subject rooted anywhere else keeps every candidate.
fn root_may_cover(site: &WriteSite, subject: &ValuePath) -> bool {
    match &subject.root {
        PathRoot::Local(name) => site.rooted_at(name.as_str()),
        PathRoot::This => site.rooted_at("this"),
        _ => true,
    }
}
