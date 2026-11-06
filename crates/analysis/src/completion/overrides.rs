//! Declaring a method a supertype already declared, whole.
//!
//! The one completion whose answer is not a name but a declaration: a person typing `setUp` in a
//! class body is not naming something, they are about to write four lines of which only one is
//! theirs. So the item writes all four and puts the cursor on the one that is.
//!
//! Offered only where a member declaration can *begin* -- a class body, with nothing of the
//! declaration written yet. Two reasons, and the second is the load-bearing one:
//!
//! - What accepting it writes includes `public function`, and after `public function ` those words
//!   are already in the file. A completion replaces the identifier it was typed over and nothing
//!   else, so there is no honest way to offer the whole declaration once part of it exists.
//! - The list is built per keystroke and a real supertype chain is a hundred methods deep. Paying
//!   for that at every statement position in every method body would be the whole cost of the
//!   feature spent where it can never be accepted.

use phrost_model::{SymbolKind, TextSize, short_name};
use phrost_view::{IndexView, MemberKind};

use phrost_model::matches_loosely;

use super::{CompletionItem, Describe, Insertion, MemberRef};
use crate::stub::Layout;

/// The methods the class around the cursor could declare, each as the stub accepting it leaves.
pub(super) fn overrides(
    view: &IndexView<'_>,
    offset: TextSize,
    typed: &str,
) -> Vec<CompletionItem> {
    let Some(class) = view.class_body_at(offset) else {
        return Vec::new();
    };
    let Some(text) = view.current_text() else {
        return Vec::new();
    };
    let layout = Layout::at(text, offset);

    // The prefix goes in rather than being applied to what comes back: what makes this cost
    // anything is rendering a declaration, and a supertype chain on a real project is a couple of
    // hundred methods deep.
    view.overridable_methods(class.as_str(), |name| matches_loosely(name, typed))
        .into_iter()
        .map(|method| CompletionItem {
            demoted: false,
            deprecated: method.deprecated,
            label: method.name.as_str().to_owned(),
            kind: SymbolKind::Method,
            // Which of the two this is, and where it comes from. Short, like every other detail
            // here, and it is the part a reader chooses between: three supertypes can each leave a
            // method of the same name, and only one of them is the one being thought of.
            detail: Some(format!(
                "{} {}",
                match method.is_required {
                    true => "implement",
                    false => "override",
                },
                short_name(method.declared_by.as_str())
            )),
            // The declaration being replaced, so the second request describes what has to be fitted
            // rather than what the stub already shows.
            describe: Some(Describe::Member(MemberRef {
                class: method.declared_by.as_str().to_owned(),
                kind: MemberKind::Method,
                name: method.name.as_str().to_owned(),
            })),
            insert: Some({
                let (text, body) = layout.stub(&method);
                Insertion {
                    text,
                    cursor: Some(body),
                }
            }),
            // The stub is the whole declaration, brackets included. There is nothing to add after.
            call: None,
            // A stub names its classes fully qualified, so it needs no `use` line to mean what it
            // says -- and an import per parameter type would be a lot of edits for one accepted item.
            import: None,
        })
        .collect()
}
