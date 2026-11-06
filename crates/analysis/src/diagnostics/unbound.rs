//! A variable read where nothing gives it a value.
//!
//! `$app;` before the line that assigns it, or `$aplication` where `$application` was meant. PHP 8
//! warns at runtime and evaluates it as null, so it is a branch that runs and does the wrong thing
//! -- and the commonest way to get one is a typo in a name, which nothing else here can see.
//!
//! The mirror of [`UnusedLocal`](super::unused::UnusedLocal), with the failure mode mirrored too: a
//! missed *binding* is what produces a false finding, and PHP creates variables in places that do not
//! look like writing. The layer below enumerates them; what matters here is that a name passed as an
//! argument counts as bound, because a by-reference parameter fills it and the call site never says
//! which parameters those are.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const UNBOUND_VARIABLE: &str = "unbound-variable";

pub(super) struct UnboundLocal;

impl Check for UnboundLocal {
    /// Like the unused check, this reads the whole file: a variable is unbound because nothing
    /// *anywhere* in its scope binds it, so text the parser skipped would read as nothing binding it.
    fn needs_a_whole_file(&self) -> bool {
        true
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for local in view.locals_never_bound() {
            out.push(Diagnostic {
                range: local.range,
                // A warning, not an error. PHP carries on with null rather than stopping, which is
                // exactly what makes it worth pointing at: the program runs and is wrong.
                severity: Severity::Warning,
                code: UNBOUND_VARIABLE,
                message: format!("Nothing gives ${} a value", local.name),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
