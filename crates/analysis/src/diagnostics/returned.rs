//! A `return` or a `yield` the declared return type refuses.
//!
//! Four of PHP's compile-time fatals -- see
//! [`refused_returns`](phrost_view::IndexView::refused_returns), where each is written down with
//! the message PHP gives and the generator rule is narrower than it looks.

use phrost_view::{IndexView, WrongReturn};

use super::{Check, Diagnostic, Severity};

pub(crate) const REFUSED_RETURN: &str = "refused-return";

pub(super) struct RefusedReturn;

impl Check for RefusedReturn {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.refused_returns() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: REFUSED_RETURN,
                message: match found.because {
                    WrongReturn::ValueFromVoid => {
                        "a void function must not return a value".to_owned()
                    }
                    WrongReturn::NothingFromTyped => {
                        "this declares a return type, so `return` needs a value".to_owned()
                    }
                    WrongReturn::ReturnFromNever => {
                        "a never-returning function must not return at all".to_owned()
                    }
                    WrongReturn::YieldFromNonGenerator => {
                        "this yields, and the declared return type cannot be a generator".to_owned()
                    }
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
