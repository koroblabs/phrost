//! A name the project's own code declares in two files.
//!
//! `Cannot redeclare class C` the moment both are loaded, and which one wins before that is an
//! accident of the autoloader -- so every answer about the name is a coin toss. See
//! [`twice_declared`](phrost_view::IndexView::twice_declared) for what is left alone: vendor, where
//! a moved class is in two packages for a release, and a guarded shim, where the guard is the
//! point.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const TWICE_DECLARED: &str = "twice-declared";

pub(super) struct TwiceDeclared;

impl Check for TwiceDeclared {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.twice_declared() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: TWICE_DECLARED,
                message: format!("{} is also declared in {}", found.class, found.other),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
