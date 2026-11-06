//! A variable read before the line that gives it a value.
//!
//! `echo $total; $total = $a + $b;`, or the commoner one -- a line moved above the assignment it
//! depends on. PHP 8 warns and evaluates the name as null, so nothing stops: the null travels into
//! whatever the expression feeds, and the report about *it* is somewhere else entirely.
//!
//! The positional sibling of [`UnboundLocal`](super::unbound::UnboundLocal). That one asks whether
//! anything in the scope binds the name at all; this one asks whether anything binds it above the
//! read, and the two never speak about the same name -- a name nothing binds is the other's finding.
//! The claim is deliberately the narrow one: every binding of the name is *below* this read, which
//! makes it a fact about order rather than a guess about which branch ran. See
//! [`phrost_view::EarlyRead`]'s own layer for what that refuses and why.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const READ_BEFORE_ASSIGNMENT: &str = "read-before-assignment";

pub(super) struct ReadBeforeAssignment;

impl Check for ReadBeforeAssignment {
    /// The evidence is an *absence* -- nothing above this read binds the name -- and the binding
    /// that would clear it is by definition below the read, which is exactly the text recovery
    /// throws away. Position cannot make this one safe, so it stays off a broken file entirely.
    fn needs_a_whole_file(&self) -> bool {
        true
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for early in view.locals_read_before_bound() {
            out.push(Diagnostic {
                range: early.range,
                // A warning for the reason the unbound check is one: PHP carries on with null
                // rather than stopping, which is what makes it worth pointing at.
                severity: Severity::Warning,
                code: READ_BEFORE_ASSIGNMENT,
                message: format!("Nothing has given ${} a value yet", early.name),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
