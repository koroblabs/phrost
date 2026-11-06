//! A format string and the values written beside it, disagreeing about a placeholder.
//!
//! Nothing fails when they do. `f('Hello @name', ['@nam' => $x])` runs, and the page says
//! `Hello @name` to whoever loaded it -- so this is a class of bug that reaches production by
//! definition, because the only thing that reports it is a person reading the output.
//!
//! Two directions, and they are not equally safe:
//!
//! - **A placeholder nothing supplies** is decided by the call alone. The message is a literal, the
//!   map is a literal, and `strtr` is given exactly what is written here -- so there is no second
//!   file, no framework contract and no runtime that could supply the value instead. Always on.
//! - **A value no placeholder uses** is the same reading in reverse and is just as certain about
//!   what happens (the value is computed and discarded). It is less certain about whether that is
//!   what the author meant: the map may be written once for two messages, or carry an entry a
//!   variant of the call reads. Measured on a production checkout it produced no wrong report, and
//!   it is on for that reason rather than by symmetry -- see the fixtures for the shapes that made
//!   it declinable.
//!
//! See `phrost_view::placeholders` for what identifies one of these calls and everything it declines
//! to read.

use phrost_view::{IndexView, PlaceholderFault};

use super::{Check, Diagnostic, Severity};

/// Named because a quick fix will want to add the missing entry.
pub(crate) const MISSING_PLACEHOLDER: &str = "missing-placeholder";
/// Named because a quick fix will want to remove the entry.
pub(crate) const UNUSED_PLACEHOLDER: &str = "unused-placeholder";

pub(super) struct PlaceholderMismatch;

impl Check for PlaceholderMismatch {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.placeholder_mismatches() {
            out.push(Diagnostic {
                range: found.range,
                // A warning in both directions: the line runs, and what is wrong is what it
                // produces. An error is reserved here for code that cannot run at all.
                severity: Severity::Warning,
                code: match found.fault {
                    PlaceholderFault::Unsupplied => MISSING_PLACEHOLDER,
                    PlaceholderFault::Unused => UNUSED_PLACEHOLDER,
                },
                message: match found.fault {
                    PlaceholderFault::Unsupplied => {
                        format!(
                            "Nothing supplies `{}`, so it is printed as written",
                            found.name
                        )
                    }
                    PlaceholderFault::Unused => {
                        format!("The message has no `{}` to replace", found.name)
                    }
                },
                // Faded rather than underlined for the value nobody reads, the way an unused import
                // is: there is nothing to stare at, only something to delete. The other direction is
                // a real defect in the output and is underlined.
                unnecessary: matches!(found.fault, PlaceholderFault::Unused),
                deprecated: false,
            });
        }
    }
}
