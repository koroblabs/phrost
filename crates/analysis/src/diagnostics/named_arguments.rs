//! A label PHP refuses at the call: `find(idd: 5)`, `find(1, id: 2)`.
//!
//! Both stop the call with an `Error` on every run that reaches it -- measured on 8.5.10, in PHP's
//! own words, which are the messages here. What gets judged and what does not is
//! [`IndexView::misnamed_arguments`]'s to say.

use phrost_view::{IndexView, Misnamed};

use super::{Check, Diagnostic, Severity};

pub(crate) const UNKNOWN_NAMED_PARAMETER: &str = "unknown-named-parameter";
pub(crate) const NAMED_PARAMETER_TWICE: &str = "named-parameter-twice";

pub(super) struct MisnamedArgument;

impl Check for MisnamedArgument {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for misnamed in view.misnamed_arguments() {
            let (code, message) = match misnamed.problem {
                Misnamed::Unknown => (
                    UNKNOWN_NAMED_PARAMETER,
                    match &misnamed.nearest {
                        Some(nearest) => format!(
                            "Unknown named parameter ${}; ${nearest} is one edit away",
                            misnamed.name
                        ),
                        None => format!("Unknown named parameter ${}", misnamed.name),
                    },
                ),
                Misnamed::Twice => (
                    NAMED_PARAMETER_TWICE,
                    format!(
                        "Named parameter ${} overwrites previous argument",
                        misnamed.name
                    ),
                ),
            };
            out.push(Diagnostic {
                range: misnamed.range,
                severity: Severity::Error,
                code,
                message,
                deprecated: false,
                unnecessary: false,
            });
        }
    }
}
