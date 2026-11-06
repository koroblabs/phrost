//! A result PHP 8.5 says should not be thrown away.
//!
//! `#[\NoDiscard]` marks a declaration whose return value is the point of calling it: a `tryLock()`
//! that answers whether it locked, a `validate()` that answers whether it passed. Writing the call
//! as a statement throws that answer away, and PHP 8.5 raises a warning for it at runtime -- one
//! whose text names the way out, `(void) tryLock();`.
//!
//! A warning and not an error, because the line still runs and the attribute is advice the author
//! of the callee wrote down. Which calls discard is [`phrost_body::discarded_calls`]'s question,
//! and whether the callee is one of these is
//! [`discarded_results`](phrost_view::IndexView::discarded_results)'.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const DISCARDED_RESULT: &str = "discarded-result";

pub(super) struct DiscardedResult;

impl Check for DiscardedResult {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for discarded in view.discarded_results() {
            out.push(Diagnostic {
                range: discarded.range,
                severity: Severity::Warning,
                code: DISCARDED_RESULT,
                message: format!(
                    "The result of {}() is declared #[\\NoDiscard] -- use it, or write `(void)` before the call",
                    discarded.callee
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
