//! A `match` over an enum that leaves one of its cases out.
//!
//! `UnhandledMatchError` is the one place PHP turns a missing case into a crash rather than a
//! default, and it is the failure a *new* enum case creates: adding one breaks every `match` over
//! that enum written without a `default`, and nothing else in a PHP toolchain says where those are.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const UNHANDLED_CASE: &str = "unhandled-case";

pub(super) struct UnhandledCase;

impl Check for UnhandledCase {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.unhandled_cases() {
            let cases = found
                .missing
                .iter()
                .map(|case| case.as_str())
                .collect::<Vec<_>>()
                .join(", ");

            out.push(Diagnostic {
                range: found.range,
                // `UnhandledMatchError` is thrown, not warned about: the arm simply is not there.
                severity: Severity::Error,
                code: UNHANDLED_CASE,
                message: format!(
                    "{} has no arm for {}, and throws UnhandledMatchError for {}",
                    found.enumeration,
                    cases,
                    match found.missing.len() {
                        1 => "it",
                        _ => "them",
                    }
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
