//! Text the parser could not make sense of.
//!
//! Reported first, and the earliest one decides how much of the rest of the file is worth checking
//! -- see [`diagnostics`](super::diagnostics). PHP parsing is error-tolerant, which is what lets an
//! editor answer about a file being typed, but recovery skips whatever it must, so a missing `;`
//! would otherwise produce a page of undefined members underneath it and bury the one line that
//! matters.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const SYNTAX: &str = "syntax";

pub(super) struct SyntaxProblems;

impl Check for SyntaxProblems {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for problem in view.syntax_problems() {
            out.push(Diagnostic {
                range: problem.range,
                severity: Severity::Error,
                code: SYNTAX,
                message: problem.message,
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
