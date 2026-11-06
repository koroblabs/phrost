//! What a template's structure gets wrong, which its PHP cannot say.
//!
//! The checks read a template through the PHP its dialect makes of it, and a block opened and never
//! closed is written there as best it can be -- so the one complaint that would name it, the
//! parser's, is about that PHP and is dropped (see [`diagnostics_asked_for`](super::diagnostics_asked_for)).
//! The dialect reads the structure from the template as written instead, which is why this takes the
//! text: the view holds only the PHP.

use phrost_view::IndexView;

use super::{AskedFor, Diagnostic, Severity};

pub(crate) const UNBALANCED_DIRECTIVE: &str = "unbalanced-directive";

/// Every block in the template `text` that does not pair up, reported where its directive is.
/// Nothing for a file that is no template.
pub fn template_diagnostics(
    view: &IndexView<'_>,
    text: &str,
    asked: AskedFor<'_>,
) -> Vec<Diagnostic> {
    if asked.switched_off(UNBALANCED_DIRECTIVE) || !view.is_template() {
        return Vec::new();
    }
    view.template_problems(text)
        .into_iter()
        .map(|(range, message)| Diagnostic {
            range,
            severity: Severity::Error,
            code: UNBALANCED_DIRECTIVE,
            message,
            unnecessary: false,
            deprecated: false,
        })
        .collect()
}
