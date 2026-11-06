//! What a declared return type says about a body, in the two ways it can be broken.
//!
//! `function f(): int { return 'x'; }` hands back the wrong thing, and `function f(): int {}` hands
//! back nothing at all. Both are a `TypeError` on the run that gets there, and both are the one
//! shape of the compatibility judgement whose two halves are written in the declaration a reader is
//! already looking at -- which is why they are one file.
//!
//! Everything about which of each is reported and which is left alone lives in
//! `IndexView::incompatible_returns` and `IndexView::missing_returns`: the generator rule, the
//! closure gap, the abstract-method rule, and why a docblock-only `@return` is not a runtime claim.
//! This is the wording.
//!
//! The third way, a value that may be a sentinel -- null, or the `false` PHP's standard library
//! hands back to mean the same -- where the declaration admits none, is `IndexView::null_returns`
//! and [`NullableReturn`] here: the return's entry in the vocabulary the argument and receiver
//! checks already speak, with the same severity split.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

/// Named for the same reason its argument sibling is: a quick fix will want to point at it.
pub(crate) const INCOMPATIBLE_RETURN: &str = "incompatible-return";

pub(crate) const MISSING_RETURN: &str = "missing-return";

pub(crate) const NULL_RETURN: &str = "null-return";
pub(crate) const POSSIBLY_NULL_RETURN: &str = "possibly-null-return";
/// A `false` that may arrive where the declaration admits none -- `reset()` on an array nothing
/// proved non-empty, handed straight back. A *certain* one is `incompatible-return`'s, which already
/// says `Cannot return false where T is declared`.
pub(crate) const POSSIBLY_FALSE_RETURN: &str = "possibly-false-return";

pub(super) struct IncompatibleReturn;

impl Check for IncompatibleReturn {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for returned in view.incompatible_returns() {
            out.push(Diagnostic {
                range: returned.range,
                // PHP raises a `TypeError`, so the function cannot finish rather than finishing
                // untidily.
                severity: Severity::Error,
                code: INCOMPATIBLE_RETURN,
                message: format!(
                    "Cannot return {} where {} is declared",
                    returned.returned, returned.declared
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

pub(super) struct MissingReturn;

impl Check for MissingReturn {
    /// The one return rule that reads the whole file. This reports the *absence* of a `return`, and
    /// recovery throws the tail of a broken file away -- so a function whose `return` was eaten by
    /// the break would be reported for not having one. Position cannot save it either: the finding
    /// is on the declaration's name, which is above the break wherever the missing statement was.
    fn needs_a_whole_file(&self) -> bool {
        true
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for missing in view.missing_returns() {
            out.push(Diagnostic {
                range: missing.range,
                // PHP raises a `TypeError` on the path that reaches the closing brace, exactly as
                // it does for a value of the wrong type.
                severity: Severity::Error,
                code: MISSING_RETURN,
                message: format!(
                    "Falling off the end returns nothing where {} is declared",
                    missing.declared
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

pub(super) struct NullableReturn;

impl Check for NullableReturn {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for returned in view.null_returns() {
            out.push(Diagnostic {
                range: returned.range,
                // A value that is null on every run is not a branch anyone gets away with; one that
                // may be is a branch nobody tested. Same split as a null argument.
                severity: match returned.certain {
                    true => Severity::Error,
                    false => Severity::Warning,
                },
                code: match (returned.null, returned.certain) {
                    (true, true) => NULL_RETURN,
                    (true, false) => POSSIBLY_NULL_RETURN,
                    (false, _) => POSSIBLY_FALSE_RETURN,
                },
                // The value's own type, because it says how the null gets in -- `?Foo` from a
                // finder, `Foo|null` from a docblock -- and that is what a reader goes and changes.
                message: match (returned.null, returned.certain) {
                    (true, true) => {
                        format!("Cannot return null where {} is declared", returned.declared)
                    }
                    // The value's own rendered type either way, which says how the sentinel gets
                    // in: `?Foo` from a finder, `Foo|false` from an array-pointer call.
                    (true, false) | (false, _) => format!(
                        "{} returned where {} is declared",
                        returned.returned, returned.declared
                    ),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
