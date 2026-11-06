//! A file the autoloader will not find.
//!
//! A class whose namespace does not match its directory is never loaded, and nothing running the
//! code says so until the moment it matters. `composer dump-autoload -o` hides it by writing a
//! classmap, so it works on the machine that dumped and fails on the one that did not -- and no
//! compiler, no type checker and no test that does not touch the class has anything to report.
//!
//! An error rather than a warning, and the two halves are the same claim: the autoloader looks for
//! a file at a path derived from the name, and it is not there. Every judgement about *whether* a
//! file is under the autoloader's territory at all belongs to
//! [`psr4_mismatch`](phrost_view::IndexView::psr4_mismatch), which says nothing about a file
//! declaring several class-likes or one declaring none.
//!
//! It does still speak about a **script** -- a `bin/*.php` run directly, which declares a class
//! nobody autoloads and is reported for it. Two on a Symfony checkout. The gate is missing rather
//! than broken, and it is missing because the cheap tests are all wrong: `use`, `namespace` and the
//! class declaration itself all reach the root body as unmodelled statements, so "the top level
//! holds something besides the class" calls every clean file a script.

use phrost_view::{IndexView, Psr4Mismatch};

use super::{Check, Diagnostic, Severity};

pub(crate) const PSR4_MISMATCH: &str = "psr4-mismatch";

pub(super) struct Psr4;

impl Check for Psr4 {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        let Some(found) = view.psr4_mismatch() else {
            return;
        };
        let (range, message) = match found {
            Psr4Mismatch::Namespace {
                range,
                declared: Some(declared),
                required,
            } => (
                range,
                format!("The autoloader looks for this file under `{required}`, not `{declared}`"),
            ),
            Psr4Mismatch::Namespace {
                range,
                declared: None,
                required,
            } => (
                range,
                format!(
                    "The autoloader looks for this file under `{required}`, and it declares no namespace"
                ),
            ),
            Psr4Mismatch::ClassName {
                range,
                declared,
                required,
            } => (
                range,
                format!(
                    "The autoloader looks for `{declared}` in `{declared}.php`, and this file is `{required}.php`"
                ),
            ),
        };
        out.push(Diagnostic {
            range,
            severity: Severity::Error,
            code: PSR4_MISMATCH,
            message,
            unnecessary: false,
            deprecated: false,
        });
    }
}
