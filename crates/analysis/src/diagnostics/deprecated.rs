//! Reaching for a declaration that says not to.
//!
//! Not an error -- the code runs, which is the point of deprecating rather than deleting. It is the
//! one finding whose whole purpose is to be read before the thing it names goes away, and it is
//! what every PHP project's upgrade is actually made of: Drupal deprecates a release ahead, Symfony
//! two, and a project that cannot see the uses finds them when the next major fails to boot.
//!
//! Silent inside a deprecated declaration, because a shim is a deprecated wrapper calling what it
//! replaced -- see the view projection, which settles which spans those are.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const DEPRECATED_USE: &str = "deprecated-use";

pub(super) struct DeprecatedUse;

impl Check for DeprecatedUse {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.deprecated_uses() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Warning,
                code: DEPRECATED_USE,
                message: format!("The {} {} is deprecated", found.kind.as_str(), found.named),
                unnecessary: false,
                // What makes a client strike the name through, which is the whole of what this
                // finding is: the code runs, and it is going away.
                deprecated: true,
            });
        }
    }
}
