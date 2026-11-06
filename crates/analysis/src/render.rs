//! Composing the PHP text a feature shows.
//!
//! Shared because hover and completion describe the same declarations, and two copies of this would
//! drift: a hover saying `public static function make()` next to a completion detail saying `make()`
//! reads as two different things being described.

use phrost_model::SymbolKind;
use phrost_view::{MemberKind, MemberModifiers};

/// A member's declaration, as it would be written.
///
/// The keyword comes from the kind rather than from the stored signature, which holds only the name
/// and the type — the index stores what it can render cheaply, and PHP's grammar is not part of it.
pub(crate) fn member_declaration(
    modifiers: Option<MemberModifiers>,
    kind: SymbolKind,
    signature: &str,
) -> String {
    let mut out = String::new();

    // An enum case has no visibility of its own -- PHP writes `case Shipped`, and the member table
    // records it as public only because everything reaching it must be.
    if let Some(modifiers) = modifiers.filter(|_| kind != SymbolKind::EnumCase) {
        out.push_str(modifiers.visibility.as_str());
        out.push(' ');
        if let Some(set) = modifiers.set_visibility {
            out.push_str(&format!("{}(set) ", set.as_str()));
        }
        for (present, word) in [
            (modifiers.is_abstract, "abstract"),
            (modifiers.is_final, "final"),
            (modifiers.is_static, "static"),
            (modifiers.is_readonly, "readonly"),
        ] {
            if present {
                out.push_str(word);
                out.push(' ');
            }
        }
    }

    match kind {
        SymbolKind::Method => out.push_str("function "),
        SymbolKind::ClassConstant => out.push_str("const "),
        SymbolKind::EnumCase => out.push_str("case "),
        _ => {}
    }

    out.push_str(signature);
    out
}

pub(crate) fn symbol_kind_of(kind: MemberKind) -> SymbolKind {
    match kind {
        MemberKind::Method => SymbolKind::Method,
        MemberKind::Property => SymbolKind::Property,
        MemberKind::ClassConstant => SymbolKind::ClassConstant,
        MemberKind::EnumCase => SymbolKind::EnumCase,
    }
}

/// Whether `name` is a stub's marker for a supertype no application code should name.
///
/// `__INTERNAL_DO_NOT_USE__StringBackedEnum` and its `Int` sibling are how the standard library
/// gives a backed enum its narrowed `$value`, `from()`, and `tryFrom()` -- real interfaces, with
/// real declarations a stub author wrote on purpose, but a name PHP itself never asks anyone to
/// write. Matched by the prefix the convention is written in rather than by an exact list of two,
/// so a sibling the standard library adds later is caught the same way a new one would be missed
/// by a list keyed to today's names.
///
/// Every place a class-like name reaches a person -- an `implements` clause, a completion
/// candidate, a workspace search hit, a hierarchy jump -- has to apply this itself: the fact that
/// the name exists is a real answer the index owes, and hiding it there would take the same
/// refusal away from a caller that has a legitimate reason to see it (a coverage sweep, `phrost
/// check`, the corpus scorer).
pub(crate) fn is_stub_internal(name: &str) -> bool {
    phrost_model::short_name(name)
        .to_ascii_lowercase()
        .starts_with("__internal_do_not_use__")
}

#[cfg(test)]
mod tests {
    use super::is_stub_internal;

    #[test]
    fn the_marker_prefix_is_matched_case_insensitively_and_by_short_name() {
        assert!(is_stub_internal("__INTERNAL_DO_NOT_USE__StringBackedEnum"));
        assert!(is_stub_internal(
            "\\Drupal\\custom\\__internal_do_not_use__stringbackedenum"
        ));
        assert!(!is_stub_internal("BackedEnum"));
        assert!(!is_stub_internal("Internal"));
    }
}
