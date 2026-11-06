//! What each statement and guard proves about a value, remembered for as long as the reader keeps it.
//!
//! Narrowing is asked per position: every read of a local walks the statements before it and the
//! guards around it, and asks each what it proves about the value. The answer is a property of the
//! statement and the value -- not of the position asking -- so a function read three hundred times
//! asked each of its statements three hundred times. A long procedural file spent most of its
//! check doing it.
//!
//! # When an answer is the same from every position
//!
//! Almost always, and the exception is exact rather than guessed. The one input that can depend on
//! where the question came from is the reader's "does this call ever return": it resolves the call
//! at the position being answered, and refuses to answer while another such question is open. So
//! the reader counts every call it was asked about that could have gone either way -- one whose
//! name something declares `never` -- and an answer whose computation saw the count move is not
//! kept. Everything else is computed from the tree and from declarations, and is the same answer
//! wherever it is asked from.
//!
//! Only for the reader's own "never". A walk that asks with a different one -- `|_, _| false`, to
//! find where a guard's claim ends -- is not given the memo at all.

use std::cell::{Cell, RefCell};

use rustc_hash::FxHashMap;

use phrost_hir::{BodyId, ExprId, StmtId};
use phrost_model::TextSize;

use crate::narrowing::Proved;
use crate::path::ValuePath;

/// The subject an answer is about, owned so it can be a key.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum SubjectKey {
    Local(Box<str>),
    Path(ValuePath),
}

/// What the reader keeps, one per request.
#[derive(Default, Debug)]
pub struct ProvedMemo {
    /// How many "never" answers could have depended on the position -- see the module docs.
    uncertain: Cell<u64>,
    /// By statement and the subject's hash, with the subject itself compared rather than cloned
    /// into the key: building an owned path per question was most of what the memo first cost,
    /// and scanning every subject a statement was asked about was most of the rest.
    statements: RefCell<Answers<(BodyId, StmtId, u64), StatementAnswer>>,
    guards: RefCell<Answers<GuardKey, Option<Proved>>>,
}

/// A guard's condition, which side of it, and the subject's hash.
type GuardKey = (BodyId, ExprId, bool, u64);

/// One table of the memo: the answers for each key, one per subject asked about.
type Answers<K, V> = FxHashMap<K, Vec<(SubjectKey, V)>>;

type StatementAnswer = Option<(TextSize, TextSize, Proved)>;

impl ProvedMemo {
    /// Says that a "never" answer was one the position could have changed.
    pub fn note_position_dependent(&self) {
        self.uncertain.set(self.uncertain.get() + 1);
    }

    /// The answer for a statement, computed by `compute` unless it is known. `hash` is the
    /// subject's [`hash_key`](crate::narrowing::Subject::hash_key), taken once by a caller asking
    /// about one subject many times.
    pub(crate) fn statement(
        &self,
        key: (BodyId, StmtId),
        subject: crate::narrowing::Subject<'_>,
        hash: u64,
        compute: impl FnOnce() -> StatementAnswer,
    ) -> StatementAnswer {
        let key = (key.0, key.1, hash);
        Self::remembered(&self.statements, &self.uncertain, key, subject, compute)
    }

    /// The answer for a guard's condition, the same way.
    pub(crate) fn guard(
        &self,
        key: (BodyId, ExprId, bool),
        subject: crate::narrowing::Subject<'_>,
        hash: u64,
        compute: impl FnOnce() -> Option<Proved>,
    ) -> Option<Proved> {
        let key = (key.0, key.1, key.2, hash);
        Self::remembered(&self.guards, &self.uncertain, key, subject, compute)
    }

    fn remembered<K: std::hash::Hash + Eq, V: Clone>(
        table: &RefCell<Answers<K, V>>,
        uncertain: &Cell<u64>,
        key: K,
        subject: crate::narrowing::Subject<'_>,
        compute: impl FnOnce() -> V,
    ) -> V {
        if let Some(known) = table.borrow().get(&key).and_then(|answers| {
            answers
                .iter()
                .find(|(about, _)| subject.is_key(about))
                .map(|(_, answer)| answer.clone())
        }) {
            return known;
        }
        let before = uncertain.get();
        let found = compute();
        if uncertain.get() == before {
            table
                .borrow_mut()
                .entry(key)
                .or_default()
                .push((subject.key(), found.clone()));
        }
        found
    }
}

/// Whether a *function* of this written name carries an assertion about one of its arguments,
/// asked of whoever can look the declaration up -- and, for a reader that keeps one, the memo the
/// narrowing walk may use. See [`crate::walk::AssertsArguments`].
pub trait Asserts {
    fn asserts(&self, hir: &phrost_hir::FileHir, name: phrost_hir::NameRefId) -> bool;

    /// The memo, for a reader whose "never" is the one it counts in. `None` keeps nothing.
    fn memo(&self) -> Option<&ProvedMemo> {
        None
    }
}

impl<F: Fn(&phrost_hir::FileHir, phrost_hir::NameRefId) -> bool> Asserts for F {
    fn asserts(&self, hir: &phrost_hir::FileHir, name: phrost_hir::NameRefId) -> bool {
        self(hir, name)
    }
}

/// A reader that knows of no assertion anywhere, for a caller with no index to ask.
#[derive(Debug)]
pub struct NoAssertions;

impl Asserts for NoAssertions {
    fn asserts(&self, _: &phrost_hir::FileHir, _: phrost_hir::NameRefId) -> bool {
        false
    }
}

/// The same answers with no memo, for a walk asking with a "never" of its own.
pub(crate) struct WithoutMemo<'a>(pub(crate) crate::walk::AssertsArguments<'a>);

impl Asserts for WithoutMemo<'_> {
    fn asserts(&self, hir: &phrost_hir::FileHir, name: phrost_hir::NameRefId) -> bool {
        self.0.asserts(hir, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lspkit_arena::ArenaId as _;

    fn key() -> (BodyId, StmtId) {
        (BodyId::from_index(0), StmtId::from_index(0))
    }

    #[test]
    fn an_answer_that_asked_a_position_dependent_never_is_not_kept() {
        let memo = ProvedMemo::default();
        let subject = crate::narrowing::Subject::Local("x");
        let mut computed = 0;

        // The first computation consulted a "never" the position could change, so nothing is kept
        // and the second computes again.
        memo.statement(key(), subject, subject.hash_key(), || {
            computed += 1;
            memo.note_position_dependent();
            None
        });
        memo.statement(key(), subject, subject.hash_key(), || {
            computed += 1;
            None
        });
        assert_eq!(computed, 2);

        // That second one consulted nothing uncertain, so it was kept and a third is answered from
        // the memo.
        memo.statement(key(), subject, subject.hash_key(), || {
            computed += 1;
            None
        });
        assert_eq!(computed, 2);

        // A different subject is a different answer.
        memo.statement(
            key(),
            crate::narrowing::Subject::Local("y"),
            crate::narrowing::Subject::Local("y").hash_key(),
            || {
                computed += 1;
                None
            },
        );
        assert_eq!(computed, 3);
    }
}
