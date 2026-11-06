//! An assignment a typed property cannot take.
//!
//! `$this->total = 'x';` where the property is declared `int`. PHP checks a typed property on every
//! write, in weak mode as well as strict, so this is a `TypeError` on the run that reaches the line
//! rather than a style note -- the same kind of claim as an incompatible argument, from the other
//! side of an `=`.
//!
//! The judgement's own precision rules decide what is said; see
//! `IndexView::incompatible_assignments`, which reports only what can be proven impossible and
//! stays quiet about every target whose declaration the index cannot account for.

use phrost_view::{IndexView, assignments::AssignedTo};

use super::{Check, Diagnostic, Severity};

/// Named because a quick fix will want to point at the declaration it argues with.
pub(crate) const INCOMPATIBLE_ASSIGNMENT: &str = "incompatible-assignment";
pub(crate) const FALSE_ASSIGNMENT: &str = "false-assignment";
pub(crate) const POSSIBLY_FALSE_ASSIGNMENT: &str = "possibly-false-assignment";

pub(super) struct IncompatibleAssignment;

impl Check for IncompatibleAssignment {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.incompatible_assignments() {
            out.push(Diagnostic {
                range: found.range,
                // A typed property write that cannot succeed is a fatal error, not a warning.
                severity: Severity::Error,
                code: INCOMPATIBLE_ASSIGNMENT,
                // A property is named in full because its declaration is usually in another file
                // and is the other half of the disagreement; an annotated local is named short,
                // because its declaration is the comment three lines above.
                message: match &found.target {
                    AssignedTo::Property { class, property } => format!(
                        "{class}::${property} takes {}, and this is {}",
                        found.declared, found.given
                    ),
                    AssignedTo::Annotated(name) => format!(
                        "${name} is declared {}, and this is {}",
                        found.declared, found.given
                    ),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

/// A write into a typed property of a value that may be `false` where it does not take one.
///
/// A warning however certain, for the reason a `false` argument is: weak mode converts it and the
/// line runs, storing `""` or `0` where the read failed. A null is `incompatible-assignment`'s.
pub(super) struct SentinelAssignment;

impl Check for SentinelAssignment {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.sentinel_assignments() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Warning,
                code: match found.certain {
                    true => FALSE_ASSIGNMENT,
                    false => POSSIBLY_FALSE_ASSIGNMENT,
                },
                message: match found.certain {
                    true => format!(
                        "Cannot write false to {}::${}, which takes {}",
                        found.class, found.property, found.declared
                    ),
                    false => format!(
                        "{}::${} takes {}, and this may be false",
                        found.class, found.property, found.declared
                    ),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
