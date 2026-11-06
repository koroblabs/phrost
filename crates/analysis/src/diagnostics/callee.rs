//! A value invoked that may not be one PHP can call.
//!
//! The fourth site in the sentinel family, beside the receiver, the subscript and the `foreach`
//! subject: `$callback()` where `$callback` is `?callable` is `Error: Value of type null is not
//! callable`.
//!
//! **Both sentinels read alike here, which is true nowhere else in the family.** Weak mode converts
//! a `false` argument and the line runs, so `possibly-false-argument` is the softer half of its
//! pair; a `false` callee is `Value of type bool is not callable` and raises `Error` whatever the
//! mode. Asked of 8.5.10 rather than read off the manual.
//!
//! What is *certainly* a sentinel is `impossible-use`, which was written for that error and names
//! the kind it found. This is the half no kind can settle.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const POSSIBLY_NULL_CALLEE: &str = "possibly-null-callee";
pub(crate) const POSSIBLY_FALSE_CALLEE: &str = "possibly-false-callee";

pub(super) struct NotCallableValue;

impl Check for NotCallableValue {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for called in view.not_callable_values() {
            let held = called
                .rendered
                .as_deref()
                .unwrap_or("a value that may not be callable");
            out.push(Diagnostic {
                range: called.range,
                // A branch nobody tested rather than a line that cannot run: what is certainly a
                // sentinel is `impossible-use`, which owns that half. Both sentinels alike, unlike
                // the argument pair -- see this module's own docs.
                severity: Severity::Warning,
                code: match called.sentinel == phrost_model::BuiltinTy::Null {
                    true => POSSIBLY_NULL_CALLEE,
                    false => POSSIBLY_FALSE_CALLEE,
                },
                message: format!("Cannot call {held}"),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
