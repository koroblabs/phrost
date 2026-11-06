//! The words of the language, where a statement may start.

use phrost_model::SymbolKind;

use phrost_model::starts_with_ignoring_case;

use super::CompletionItem;

/// The words that can start a PHP statement.
///
/// One list rather than one per place, because telling a class body from a function body lexically
/// means finding the nearest unmatched `{` and reading what came before it -- and the cost of
/// getting it wrong is one row a person filters out with the next letter. Case-insensitive, as PHP
/// is: `RETURN` is a return.
///
/// Only the words that *start* something. `as`, `insteadof` and the operators are written inside a
/// statement, where this list is not offered at all.
const STATEMENT_KEYWORDS: &[&str] = &[
    "abstract",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "declare",
    "default",
    "do",
    "echo",
    "else",
    "elseif",
    "enum",
    "final",
    "finally",
    "for",
    "foreach",
    "function",
    "global",
    "goto",
    "if",
    "include",
    "interface",
    "namespace",
    "print",
    "private",
    "protected",
    "public",
    "readonly",
    "require",
    "return",
    "static",
    "switch",
    "throw",
    "trait",
    "try",
    "unset",
    "use",
    "var",
    "while",
    "yield",
];

/// Keywords matching what has been typed, where a statement may start.
pub(super) fn keywords(prefix: &str) -> Vec<CompletionItem> {
    STATEMENT_KEYWORDS
        .iter()
        .filter(|word| starts_with_ignoring_case(word, prefix))
        .map(|word| CompletionItem {
            demoted: false,
            deprecated: false,
            label: (*word).to_owned(),
            kind: SymbolKind::Keyword,
            // Nothing to say about a keyword that the word does not already say.
            detail: None,
            describe: None,
            insert: None,
            call: None,
            import: None,
        })
        .collect()
}
