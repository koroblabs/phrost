//! A docblock that claims to implement something nothing will dispatch to it.
//!
//! Precise for one reason: the author *said so*. A framework whose runtime finds an implementation
//! by its name -- Drupal's, where `hook_form_alter` runs whatever `<extension>_form_alter` each
//! installed extension declares -- makes the docblock and the name two halves of one statement, and
//! when they disagree the function silently does nothing. Nothing else notices: it parses, it type
//! checks, its tests pass if anyone calls it directly, and it never runs.
//!
//! Found on one real checkout: a function in `app_region_notice.module` documented
//! `Implements hook_notifications_alter().` and named `app_region_notifications_alter`. There is no
//! extension called `app_region`, so the notification types it returns were never registered.
//!
//! What keeps it precise is that everything undecidable is silence. The name has to be provably
//! unreachable -- no installed extension can own it -- and every allowance the convention makes,
//! including the uppercase placeholders that make `app_core_form_views_exposed_form_alter` a correct
//! `hook_form_FORM_ID_alter`, is applied first. See the projection this reads for what those are.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const MISNAMED_IMPLEMENTATION: &str = "misnamed-implementation";
pub(crate) const UNDECLARED_IMPLEMENTATION: &str = "undeclared-implementation";

pub(super) struct MisnamedImplementation;

impl Check for MisnamedImplementation {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for claim in view.documentation_claims() {
            if claim.reachable {
                continue;
            }
            out.push(Diagnostic {
                // The function's own name, which is the half that has to change: the docblock states
                // the intention and the name is what fails to carry it out.
                range: claim.name_range,
                severity: Severity::Warning,
                code: MISNAMED_IMPLEMENTATION,
                message: format!(
                    "{}() is documented as implementing {}() and is not named for it, so nothing \
                     will call it",
                    claim.name, claim.implements
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

/// A docblock that claims to implement something that does not exist.
///
/// The other half of [`MisnamedImplementation`], and for a long time the wrong half. Both are about
/// a function that silently never runs; that one is about the *name* being unreachable, and this one
/// about there being nothing to reach. The commonest cause is a major upgrade: a framework removes
/// an extension point, the implementations of it stay, and nothing anywhere says so -- the code
/// parses, type checks, and is dead.
///
/// # Why this could not be shipped until dispatches were indexed
///
/// Measured before they were: **7 findings, 1 of them real**. The other six were extension points
/// the project declares nowhere and invokes itself, through
/// `$this->moduleHandler->invokeAll('notifications_alter')` and four more like it -- names that have
/// no declaration because a call is the whole of what declares them. Six wrong out of seven is the
/// ratio that teaches a reader to stop reading the list, which is why it was refused rather than
/// tuned.
///
/// So what makes this reportable is not this file. It is that `exists` now asks two tables: what
/// declares the name, and what dispatches it.
pub(super) struct UndeclaredImplementation;

impl Check for UndeclaredImplementation {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for claim in view.documentation_claims() {
            if claim.exists {
                continue;
            }
            out.push(Diagnostic {
                // The function's own name, as the misnaming report uses: the docblock states an
                // intention about something that is not there, and what a reader has to decide is
                // what to do with this function.
                range: claim.name_range,
                severity: Severity::Warning,
                code: UNDECLARED_IMPLEMENTATION,
                message: format!(
                    "nothing declares or dispatches {}(), so {}() implements nothing and never runs",
                    claim.implements, claim.name
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
