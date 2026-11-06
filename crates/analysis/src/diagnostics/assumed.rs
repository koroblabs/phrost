//! What a framework contract decided not to report, said out loud.
//!
//! The one check here that reports nothing wrong. A declaration says `?string`, a framework
//! guarantees the null does not arrive in any state a project's code is in, and phrost reads the
//! value as a `string` -- which is why the database layer and the entity API do not produce a
//! finding on every line that uses them. On the reference checkout those contracts suppress 118 of
//! 379 null-safety findings.
//!
//! That is a decision made on somebody's behalf, and a project is entitled to see it. Two things
//! make saying it honest rather than noise. It is off unless asked for, because the measurement the
//! contracts were written from is what makes them right by default. And it covers only the
//! contracts that hide a **real** sentinel: where the framework's own source proves the branch
//! cannot run -- `Select::execute()`'s `NULL` behind a `preExecute()` that returns `TRUE` or throws
//! -- there is nothing being decided and nothing to disclose.
//!
//! A note rather than a warning: the code is not wrong, the checker is being transparent about its
//! own reading. Not a *hint* either, which is what this reached for first -- Zed keeps hints out of
//! the diagnostics panel, so the disclosure a project switched on to read arrived and was invisible.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const ASSUMED_NON_NULL: &str = "assumed-non-null";

pub(super) struct AssumedNonNull;

impl Check for AssumedNonNull {
    /// Off unless a project asks. A project running phrost beside PHPStan at level 8 wants these,
    /// and a project that turned the contracts' benefit into the reason it keeps the checker on
    /// does not.
    fn asked_for(&self) -> Option<&'static str> {
        Some(ASSUMED_NON_NULL)
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for assumed in view.assumed_non_nulls() {
            out.push(Diagnostic {
                range: assumed.range,
                severity: Severity::Note,
                code: ASSUMED_NON_NULL,
                // Every part of the claim is on the line or one lookup away: which call, which
                // declaration the contract was written against, and the state the sentinel really
                // comes from. A reader can go and check all three, which is what separates a
                // disclosure from an assertion.
                message: format!(
                    "{}() is read as never {}, though \\{}::{}() declares it: {} only when {}",
                    assumed.name,
                    assumed.sentinel,
                    assumed.declarer,
                    assumed.name,
                    assumed.sentinel,
                    assumed.only_when
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
