//! An input a command reads by a name its declaration does not write.
//!
//! `$this->option('queu')` where the `$signature` writes `{--queue=}` is an exception the moment
//! the line runs -- `The "queu" option does not exist.` -- and a string nothing else in the toolchain
//! reads. An error, as an unregistered id in a closed catalog is: the declaration is the whole list,
//! and `IndexView::undeclared_inputs` leaves alone every class whose declaration is not.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const UNDEFINED_INPUT: &str = "undefined-input";

pub(super) struct UndefinedInput;

impl Check for UndefinedInput {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        out.extend(view.undeclared_inputs().into_iter().map(|used| {
            let declares = format!("The signature declares no {} `{}`", used.reader, used.name);
            Diagnostic {
                range: used.range,
                severity: Severity::Error,
                code: UNDEFINED_INPUT,
                message: match &used.nearest {
                    Some(near) => format!("{declares}. Did you mean `{near}`?"),
                    None => declares,
                },
                unnecessary: false,
                deprecated: false,
            }
        }));
    }
}
