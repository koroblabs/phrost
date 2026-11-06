//! Where a value is invoked and may not be one that can be.
//!
//! The fourth of the family: the receiver check asks whether the thing left of `->` is there, the
//! subscript pair asks it of a container, `foreach` asks it of what it walks, and this asks it of
//! the value between `$` and `()`. `$callback()` where `$callback` is `?callable` is
//! `Error: Value of type null is not callable`, and nothing here said so.
//!
//! # Both sentinels are fatal, which the rest of the family cannot say
//!
//! A `false` *argument* is converted by weak mode and the line runs, so it is a warning however
//! certain. A `false` **callee** is not: PHP answers `Value of type bool is not callable` and
//! raises `Error` whatever the mode -- run against 8.5.10 rather than read off the manual. So the
//! two sentinels take the same severity split here, which they do nowhere else.
//!
//! Found against PHPStan on Laravel's own source: 88 of its 107 `callable.nonCallable` findings are
//! a `?callable` invoked without a guard, which is what `when()` and `unless()` are written as.
//!
//! # The certain half belongs to somebody else
//!
//! A value whose *kind* is `null` or `bool` is already `impossible-use`, which was written for
//! `Value of type int is not callable` and names the kind it found. This reports only what *may*
//! be a sentinel, which is the half no kind can settle.

use phrost_hir::{FileHir, body::ExprData};
use phrost_model::{BuiltinTy, TextRange};

use crate::{
    references::SentinelReach,
    view::{FOLLOW_LIMIT, IndexView},
};

/// One invocation of a value that may not be callable.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NotCallable {
    /// The callee as written, which is what a report underlines.
    pub range: TextRange,
    /// What it holds, rendered -- the evidence, on the line the claim sits on.
    pub rendered: Option<String>,
    /// Which sentinel reaches it. `Null` is read first where both can arrive, because it is the
    /// one a person guards against.
    pub sentinel: BuiltinTy,
}

impl IndexView<'_> {
    /// Every `$value()` in the current file whose value may be a sentinel.
    ///
    /// Empty when the file is not open, which is the shape of the two tiers rather than a fallback:
    /// a call lives in a body, and bodies exist for open buffers only.
    pub fn not_callable_values(&self) -> Vec<NotCallable> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = hir
            .expressions()
            .filter_map(|(_, expr)| match expr {
                ExprData::IndirectCall { callee, .. } => self.callable_value(hir, *callee),
                _ => None,
            })
            .collect::<Vec<_>>();
        out.sort_by_key(|found| found.range.start());
        out
    }

    fn callable_value(&self, hir: &FileHir, callee: phrost_hir::ExprId) -> Option<NotCallable> {
        let range = hir.expr(callee).range();
        let path = phrost_body::value_path_of(hir, callee)?;
        let held = self.resolve_path(range.start(), hir, &path, FOLLOW_LIMIT)?;
        // `mixed` answers `Possible` to the null question without meaning it -- see the same
        // exception in `not_iterable`, and `states_non_null` for why it is named there too.
        if self.kind_of_held(hir, &held) == Some(BuiltinTy::Mixed) {
            return None;
        }
        // The *possible* half alone. A value whose kind is certainly `null` or `bool` is already
        // named by [`impossible_use`](crate::ImpossibleUse), which was written for exactly that
        // error and says which kind it found -- so reporting it here too would be two findings on
        // one line and a second constructor of one rule.
        let sentinel = match (
            self.null_reach_of(hir, &held),
            self.false_reach_of(hir, &held),
        ) {
            (SentinelReach::Possible, _) => BuiltinTy::Null,
            (_, SentinelReach::Possible) => BuiltinTy::False,
            _ => return None,
        };
        Some(NotCallable {
            range,
            rendered: self.render_held(range.start(), hir, &held, crate::variables::Naming::Short),
            sentinel,
        })
    }
}
