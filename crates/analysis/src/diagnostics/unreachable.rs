//! Statements written after a block control cannot get past.
//!
//! `return $a; $b = 1;` -- the second line never runs. Not wrong on its own, which is why it is
//! faded rather than underlined, but it is almost always the symptom of something else: a mismerge,
//! an inverted guard, a `return` moved up while the code under it stayed.
//!
//! Which runs and which cannot is decided in `IndexView::unreachable_code`, including the two
//! shapes it steps around -- PHP hoists a declaration whatever it is written after, and a `goto`
//! label is reached from somewhere the flow walk cannot see.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const UNREACHABLE_CODE: &str = "unreachable-code";

pub(super) struct UnreachableCode;

impl Check for UnreachableCode {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.unreachable_code() {
            out.push(Diagnostic {
                range: found.range,
                // Nothing about the program is wrong: this code does not run, and code that does
                // not run cannot misbehave. What it asks for is deletion, which is what the faded
                // rendering asks for too.
                severity: Severity::Warning,
                code: UNREACHABLE_CODE,
                message: "Nothing reaches this code".to_owned(),
                unnecessary: true,
                deprecated: false,
            });
        }
    }
}
