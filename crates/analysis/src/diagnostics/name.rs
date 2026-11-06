//! A class-like name that names nothing.
//!
//! The defect a refactor leaves behind: a moved class, a deleted one, a `use` line nobody updated.
//! At runtime it is a fatal error, and unlike a missing member there is nothing dynamic about it --
//! a name either resolves through this file's imports and namespace or it does not.
//!
//! # A name in a docblock
//!
//! Reported under its own code, `doc-undefined-type`, and as a warning: in a docblock a missing name
//! is a wrong annotation rather than a fatal error, and a project may disagree with the rule without
//! disagreeing with the rest. The docblock type language also has names PHP does not -- a
//! `@template` parameter, `self` and `static`, PHPStan's dashed pseudo-types -- and those are left
//! alone, as is a `@see`, which points a reader somewhere rather than stating a type.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

/// Named because `actions::import` offers the fix for it.
pub(crate) const UNDEFINED_TYPE: &str = "undefined-type";
/// A class-like name in a docblock type that names nothing.
pub(crate) const DOC_UNDEFINED_TYPE: &str = "doc-undefined-type";

/// The words of the docblock type language that arrive as names and name no class -- `Closure`
/// among them, since `Closure(int): string` is a callable signature's keyword and means `\Closure`
/// in any namespace, imported or not.
const NOT_CLASSES: &[&str] = &[
    "self", "static", "parent", "$this", "null", "true", "false", "Closure",
];

pub(super) struct UndefinedType;

impl Check for UndefinedType {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        let templates = view.templates_declared();
        for reference in view.name_references() {
            // A name the file tests for is one the author has already said may be absent.
            if reference.resolved.is_some() || reference.existence_tested {
                continue;
            }
            let path = reference.path.as_str();
            let (code, severity, message) = match reference.in_docblock {
                false => (
                    UNDEFINED_TYPE,
                    Severity::Error,
                    format!("Undefined type {path}"),
                ),
                // A name written all in capitals is a constant -- `($n is INF ? ...)` -- and a
                // constant is no class whatever it resolves to.
                true if reference.cross_reference
                    || path.contains('-')
                    || path
                        .chars()
                        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
                    || templates.iter().any(|template| template == path)
                    || NOT_CLASSES
                        .iter()
                        .any(|word| path.eq_ignore_ascii_case(word)) =>
                {
                    continue;
                }
                true => (
                    DOC_UNDEFINED_TYPE,
                    Severity::Warning,
                    format!("Undefined type {path} in a docblock"),
                ),
            };

            out.push(Diagnostic {
                range: reference.range,
                severity,
                code,
                message,
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
