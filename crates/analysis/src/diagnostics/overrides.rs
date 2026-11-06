//! A method that does not fit the one it overrides.
//!
//! A fatal error at class load, before any of the class runs -- so unlike most findings this one is
//! not about a branch nobody took. The declaration it has to fit is in another file, which is why
//! this check lives here and not in a linter.
//!
//! Only what is exact. PHP's variance rules for parameter and return *types* need a subtyping
//! relation this does not have, and half of one reports correct code.

use phrost_model::Visibility;
use phrost_view::{Incompatible, IndexView};

use super::{Check, Diagnostic, Severity};

pub(crate) const BAD_OVERRIDE: &str = "bad-override";
pub(crate) const NOTHING_TO_OVERRIDE: &str = "nothing-to-override";

pub(super) struct BadOverride;

/// A `#[\Override]` with nothing above it to override.
///
/// Its own code rather than a sixth reason of [`BadOverride`], because the two say different things
/// to a reader: that one is "your declaration does not fit the one above it", and this is "there is
/// no declaration above it at all". A person filtering one may well want the other.
pub(super) struct NothingToOverride;

impl Check for NothingToOverride {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for empty in view.empty_overrides() {
            out.push(Diagnostic {
                range: empty.range,
                // A fatal at class load, like its neighbour: the class does not load at all.
                severity: Severity::Error,
                code: NOTHING_TO_OVERRIDE,
                message: match empty.is_constructor {
                    _ if empty.is_property => format!(
                        "${} is #[\\Override] and nothing above it declares that property",
                        empty.method
                    ),
                    true => format!(
                        "{}() is #[\\Override] and a constructor, which PHP never counts as overriding",
                        empty.method
                    ),
                    false => format!(
                        "{}() is #[\\Override] and nothing above it declares that name",
                        empty.method
                    ),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

impl Check for BadOverride {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.bad_overrides() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: BAD_OVERRIDE,
                message: format!(
                    "{}() cannot override {}::{}(): {}",
                    found.method,
                    found.parent,
                    found.method,
                    explain(&found.because)
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

fn explain(because: &Incompatible) -> String {
    match because {
        Incompatible::ParentIsFinal => "the declaration above it is final".to_owned(),
        Incompatible::StaticnessChanged { was_static: true } => {
            "the declaration above it is static and this is not".to_owned()
        }
        Incompatible::StaticnessChanged { was_static: false } => {
            "this is static and the declaration above it is not".to_owned()
        }
        Incompatible::VisibilityReduced { was, now } => {
            format!("{} cannot narrow to {}", word(*was), word(*now))
        }
        Incompatible::FewerParameters { was, now } => format!(
            "it takes {now} parameter{} where {was} {} declared",
            match now {
                1 => "",
                _ => "s",
            },
            match was {
                1 => "is",
                _ => "are",
            }
        ),
        Incompatible::AddedRequired { name } => {
            format!("${name} is required here and the declaration above it has no such parameter")
        }
    }
}

fn word(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Public => "public",
        Visibility::Protected => "protected",
        Visibility::Private => "private",
    }
}
