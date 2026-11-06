//! A `foreach` handed something it cannot walk.
//!
//! The third rule about reading a container, beside the two in
//! [`subscripts`](super::subscripts): that file asks whether a value holds elements and whether it
//! is there at all, and this asks the same two questions of the one statement that reads a
//! container whole.
//!
//! PHP does not stop here. `foreach() argument must be of type array|object, null given` is a
//! warning and the body simply never runs -- so every code below is a warning, exactly as the
//! subscript pair is, and for the same reason: a branch that runs and is wrong rather than one
//! that stops.

use phrost_view::{IndexView, NotIterable};

use super::{Check, Diagnostic, Severity};

pub(crate) const NULL_ITERATION: &str = "null-iteration";
pub(crate) const POSSIBLY_NULL_ITERATION: &str = "possibly-null-iteration";
pub(crate) const POSSIBLY_FALSE_ITERATION: &str = "possibly-false-iteration";
pub(crate) const FALSE_ITERATION: &str = "false-iteration";

pub(super) struct NotIterableLoop;

impl Check for NotIterableLoop {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for loop_ in view.not_iterable_loops() {
            let held = loop_
                .rendered
                .as_deref()
                .unwrap_or("a value that cannot be iterated");
            out.push(Diagnostic {
                range: loop_.range,
                severity: Severity::Warning,
                code: match loop_.why {
                    NotIterable::Null { certain: true } => NULL_ITERATION,
                    NotIterable::Null { certain: false } => POSSIBLY_NULL_ITERATION,
                    NotIterable::False { certain: true } => FALSE_ITERATION,
                    NotIterable::False { certain: false } => POSSIBLY_FALSE_ITERATION,
                },
                message: format!("Cannot iterate {held}"),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
