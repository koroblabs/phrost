//! Every way through the `if` a name's possible writes are in that runs none of them, and what each
//! proves about the name.
//!
//! The path around a set of writes is a *set* of paths, and what holds there is what holds on every
//! one of them. `if ($a) { if ($x === null) { $x = 'd'; } }` is skipped where `!$a`, which tests
//! nothing about `$x`, and where `$a && $x !== null` -- so nothing is proved, and reading only the
//! guard the write sits under said `$x` could not be null there. The two connectives are kept
//! apart: on one path every guard held, so any one of them is true of it; across paths only what
//! all of them imply is. Symfony's `FixedWindowLimiter` is the shape that settles which is which --
//! one path proves the window is a `CalendarAlignedWindow`, the other that it is a `Window`, and
//! what both say is that there is one.
//!
//! Only a tree of `if`s, which is the construct written with a write per arm. Anything else holding
//! a write -- a `switch`, a `try`, a loop, a write inside a condition or behind a `&&` -- gives the
//! question back, and the reading by one guard per write answers it as before.

use lspkit_arena::IdSlice;
use phrost_hir::{FileHir, StmtId, body::StmtData};

use crate::{
    facts::BodyFacts,
    locals::MaybeWrite,
    narrowing::{PROVED_DEPTH, Subject},
    path::{Otherwise, Refused},
    reach::Reach,
    scope::Scope,
    walk::{AssertsArguments, statement_leaves},
};

/// How many ways through a construct are read before the question is given back. A dozen nested
/// `if`s is past anything a person writes around one value, and each way is a handful of claims.
const MOST_PATHS: usize = 32;

/// One way through, as the claims along it in the order they were made.
type Path = Vec<Otherwise>;

impl Scope {
    /// What the path that skipped every one of `maybe` proved, read off every way around them -- or
    /// `None` where the writes are not in a tree of `if`s this walks.
    pub(crate) fn otherwise_by_paths(
        &self,
        hir: &FileHir,
        maybe: &[MaybeWrite<'_>],
        subject: Subject<'_>,
        asserts: AssertsArguments<'_>,
    ) -> Option<Otherwise> {
        let since = maybe.first()?.since;
        let construct = BodyFacts::of(hir).innermost_statement(hir, self.body(), since)?;
        if !matches!(hir.stmt(construct), StmtData::If { .. })
            || hir.stmt(construct).range().start() != since
        {
            return None;
        }
        let walk = Ways {
            scope: self,
            hir,
            maybe,
            subject,
            asserts,
        };
        let mut agreed: Option<Otherwise> = None;
        for path in walk.through(construct)? {
            let claim = Self::held_on(&path);
            agreed = Some(match agreed {
                None => claim,
                Some(had) => Self::held_on_either(had, claim),
            });
            if agreed == Some(Otherwise::Unproved) {
                break;
            }
        }
        // No way round them at all: every one either wrote or left.
        Some(agreed.unwrap_or(Otherwise::Impossible))
    }

    /// What holds on one way through, where every claim made along it held.
    ///
    /// Any of them is true there, and one is all this carries -- so the one kept is the one a reader
    /// most needs: that the value was there, and of those the latest, which is the narrower when one
    /// guard is inside another. An assertion is a claim this vocabulary cannot weigh against the
    /// others and is kept only where nothing else was said.
    fn held_on(path: &Path) -> Otherwise {
        let stated = || {
            path.iter()
                .filter(|claim| !matches!(claim, Otherwise::Unproved | Otherwise::Asserted(_)))
        };
        stated()
            .rev()
            .find(|claim| claim.rules_out_null())
            .or_else(|| stated().next_back())
            .or_else(|| {
                path.iter()
                    .rfind(|claim| matches!(claim, Otherwise::Asserted(_)))
            })
            .cloned()
            .unwrap_or(Otherwise::Unproved)
    }

    /// What holds on one way or the other: the same claim, or the value being there where both say
    /// it was, or nothing.
    fn held_on_either(had: Otherwise, claim: Otherwise) -> Otherwise {
        if had == claim {
            return had;
        }
        match had.rules_out_null() && claim.rules_out_null() {
            true => Otherwise::NotNull {
                refused: Refused::NullOnly,
            },
            false => Otherwise::Unproved,
        }
    }
}

/// The walk, with what every step of it asks.
struct Ways<'a, 'f> {
    scope: &'a Scope,
    hir: &'a FileHir,
    maybe: &'a [MaybeWrite<'f>],
    subject: Subject<'a>,
    asserts: AssertsArguments<'a>,
}

impl Ways<'_, '_> {
    /// The ways through one statement that run none of the writes, or `None` for a statement holding
    /// a write in a shape this does not walk.
    fn through(&self, statement: StmtId) -> Option<Vec<Path>> {
        let hir = self.hir;
        let range = hir.stmt(statement).range();
        let writes = self
            .maybe
            .iter()
            .filter(|write| range.contains(write.at))
            .collect::<Vec<_>>();
        if writes.is_empty() {
            if statement_leaves(hir, statement, true, &|_, _| false) {
                return Some(Vec::new());
            }
            // What it proves on the way past -- an `if` whose arm leaves, an assertion -- which is
            // how the arm around a write says what the value was where it was not replaced.
            let claim = self
                .scope
                .proved_by(
                    hir,
                    statement,
                    self.subject,
                    &|_, _| false,
                    self.asserts,
                    PROVED_DEPTH,
                )
                .map(|(_, _, proved)| Scope::otherwise_of(proved, range.start()));
            return Some(vec![claim.into_iter().collect()]);
        }
        match hir.stmt(statement) {
            StmtData::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                // A write in the condition runs on every way through, which no arm says.
                let tested = hir.expr(*condition).range();
                if writes.iter().any(|write| tested.contains(write.at)) {
                    return None;
                }
                let mut ways = Vec::new();
                for (arm, positive) in [(*then_branch, true), (*else_branch, false)] {
                    let entered = self
                        .scope
                        .proved_by_condition(hir, *condition, self.subject, positive, self.asserts)
                        .map_or(Otherwise::Unproved, |proved| {
                            Scope::otherwise_of(proved, tested.start())
                        });
                    for path in self.list(arm)? {
                        ways.push(std::iter::once(entered.clone()).chain(path).collect());
                    }
                }
                (ways.len() <= MOST_PATHS).then_some(ways)
            }
            StmtData::Block { body, .. } => self.list(*body),
            // The write is the statement, or inside it: none of the ways through skips it unless one
            // of its writes is behind a `&&` or a `??`, which this does not take apart.
            StmtData::Expr { .. } => {
                let facts = BodyFacts::of(hir);
                writes
                    .iter()
                    .all(|write| {
                        facts.reach_of(hir, write.site, range.end(), &|_, _| false)
                            == Some(Reach::Certain)
                    })
                    .then(Vec::new)
            }
            _ => None,
        }
    }

    /// The ways through a list, one statement after another: each way into a statement goes on by
    /// every way through it.
    fn list(&self, statements: IdSlice<StmtId>) -> Option<Vec<Path>> {
        let mut ways: Vec<Path> = vec![Vec::new()];
        for &statement in self.hir.stmt_list(statements) {
            let through = self.through(statement)?;
            let mut next = Vec::with_capacity(ways.len() * through.len());
            for way in &ways {
                for tail in &through {
                    next.push(way.iter().chain(tail).cloned().collect());
                }
            }
            if next.len() > MOST_PATHS {
                return None;
            }
            ways = next;
            if ways.is_empty() {
                break;
            }
        }
        Some(ways)
    }
}
