//! A string read at an offset no string offset can be.
//!
//! `$s['key']` raises `Cannot access offset of type string on string` -- a `TypeError` that stops
//! the line, not the warning a subscripted `int` gives. See
//! [`bad_string_offsets`](phrost_view::IndexView::bad_string_offsets): what makes it certain is the
//! literal, because PHP reads a numeric string as a position and `$s['1']` is a character.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const BAD_STRING_OFFSET: &str = "bad-string-offset";

pub(super) struct BadStringOffset;

impl Check for BadStringOffset {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.bad_string_offsets() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: BAD_STRING_OFFSET,
                message: format!(
                    "'{}' is not a position in a string, and PHP refuses the offset",
                    found.key
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
