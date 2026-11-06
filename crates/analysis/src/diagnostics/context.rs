//! A name written where the language gives it nothing to refer to.
//!
//! `Using $this when not in object context` and its three neighbours are fatal the moment the line
//! runs, and each is stated entirely by the file -- see
//! [`out_of_context`](phrost_view::IndexView::out_of_context) for what has an object context and
//! what does not, which was run against PHP 8.5.10 rather than assumed.

use phrost_view::{IndexView, Written};

use super::{Check, Diagnostic, Severity};

pub(crate) const OUT_OF_CONTEXT: &str = "out-of-context";

pub(super) struct OutOfContext;

impl Check for OutOfContext {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.out_of_context() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: OUT_OF_CONTEXT,
                message: match found.written {
                    Written::This => "$this is written where there is no object".to_owned(),
                    Written::SelfTy => "self is written outside a class".to_owned(),
                    Written::StaticTy => "static is written outside a class".to_owned(),
                    Written::Parent => {
                        "parent is written in a class that extends nothing".to_owned()
                    }
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
