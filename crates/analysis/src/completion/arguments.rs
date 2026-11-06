//! The parameters of the call being written, offered as named arguments.

use phrost_model::{SymbolKind, TextSize};
use phrost_view::IndexView;

use phrost_model::starts_with_ignoring_case;

use super::{CompletionItem, Insertion};

/// The parameters of the call being written, as named arguments.
///
/// First in the list, because at an argument position they are the more specific answer: a name
/// matching the prefix is one of thousands, and a parameter of the callee is one of five.
pub(super) fn named_arguments(
    view: &IndexView<'_>,
    offset: TextSize,
    prefix: &str,
) -> Vec<CompletionItem> {
    view.free_parameters(offset)
        .into_iter()
        .filter(|param| starts_with_ignoring_case(param.name.as_str(), prefix))
        .map(|param| CompletionItem {
            demoted: false,
            deprecated: false,
            // The colon is in the label, because that is what makes the row read as a named
            // argument rather than as another class called `stage`.
            label: format!("{}:", param.name),
            kind: SymbolKind::Variable,
            detail: param.ty,
            // The declaration is the callee's, and signature help is already showing it.
            describe: None,
            // The space too: a named argument is always followed by its value, and every client
            // leaves the cursor after what it inserted.
            insert: Some(Insertion::plain(format!("{}: ", param.name))),
            call: None,
            import: None,
        })
        .collect()
}
