//! A class that does not declare what its supertypes require.
//!
//! `class Foo implements Bar {}` with no `Bar::baz()` anywhere in the hierarchy is a fatal error at
//! *class load* -- PHP refuses the declaration before a line of it runs -- so this ranks with
//! [`BadOverride`](super::overrides::BadOverride) rather than with a check about a branch nobody
//! took. What the class has to provide is written in other files, which is why it is here and not in
//! a linter.
//!
//! One finding per class, listing every name. The class is what fails to load, and a reader given
//! four separate reports on one declaration has one repair to make either way.
//!
//! Everything the judgement declines to claim -- an interface this index does not hold, a
//! requirement no `extends` or `implements` was written for, a trait's abstract method -- is in
//! `phrost_view::unimplemented`.

use phrost_view::{IndexView, RequiredMethod};

use super::{Check, Diagnostic, Severity};

pub(crate) const UNIMPLEMENTED_METHOD: &str = "unimplemented-method";

pub(super) struct UnimplementedMethod;

impl Check for UnimplementedMethod {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.unimplemented_methods() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: UNIMPLEMENTED_METHOD,
                message: format!(
                    "{} does not implement {}",
                    found.class,
                    named(&found.missing)
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

/// The missing methods, each qualified by what asks for it.
///
/// Qualified even when there is one, because the name alone does not say where to look for the
/// signature to copy -- and that is the next thing the reader needs.
fn named(missing: &[RequiredMethod]) -> String {
    missing
        .iter()
        .map(|method| format!("{}::{}()", method.required_by, method.name))
        .collect::<Vec<_>>()
        .join(", ")
}
