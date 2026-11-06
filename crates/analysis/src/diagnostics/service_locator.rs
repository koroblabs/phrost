//! A static call into the container, from a class the container builds.
//!
//! `File::load()` inside a form, `\Drupal::service('...')` inside a controller: each fetches a
//! dependency through the side door of a class whose front door -- its constructor, fed by
//! `create()` or a service definition -- could have been handed it. Nothing breaks, so this is a
//! house rule and asked for rather than always on; the reference project's own history is a commit
//! moving two view builders off `File::load()` by hand.
//!
//! What counts as either half is the framework's, through `ContainerRule`; see the projection for
//! which methods are left out and why.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const SERVICE_LOCATOR_CALL: &str = "service-locator-call";

pub(super) struct ServiceLocatorCall;

impl Check for ServiceLocatorCall {
    fn asked_for(&self) -> Option<&'static str> {
        Some(SERVICE_LOCATOR_CALL)
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for reach in view.container_reaches() {
            out.push(Diagnostic {
                range: reach.range,
                severity: Severity::Warning,
                code: SERVICE_LOCATOR_CALL,
                message: format!(
                    "`{}::{}()` fetches from the container inside `{}`, which the container builds -- inject {} instead",
                    phrost_model::short_name(reach.class.as_str()),
                    reach.method,
                    phrost_model::short_name(&reach.built),
                    reach.instead,
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
