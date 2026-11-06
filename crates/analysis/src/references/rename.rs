//! What a new name replaces.
//!
//! A reference is not always just the name. `use App\Repo;` and `new App\Repo()` write a path, and
//! replacing the whole of it with `Ledger` produces `use Ledger;` -- an import of a class in the
//! global namespace, which is not what anyone asked for. Only the last segment is the name.

use phrost_model::{TextRange, TextSize};
use phrost_view::IndexView;

/// The written span a rename from this cursor would replace, before the last segment is taken off
/// it.
///
/// What `prepareRename` answers with, and the reason it lives here rather than in the protocol
/// layer: the dialogue must open exactly where the rename edits, and the two used to be built
/// separately. A class named by a *string* holds no name reference and sits at no `->`, so the
/// protocol's own list could not see one -- and the dialogue refused to open on
/// `is_a($x, 'App\Repo')` while a rename from that very position edited forty sites in eighteen
/// files.
///
/// The narrowest of the candidates, which is the rule [`phrost_view::IndexView::cursor_target`]
/// states for the same question: the innermost thing is what was clicked.
pub fn rename_span(view: &IndexView<'_>, offset: TextSize) -> Option<TextRange> {
    let declared = view
        .declaration_at(offset)
        .map(|location| location.name_range);
    view.name_references()
        .into_iter()
        .map(|reference| reference.range)
        .chain(view.member_accesses().iter().map(|access| access.range))
        .chain(
            view.class_named_by_a_string_at(offset)
                .into_iter()
                .chain(view.own_method_named_by_a_string_at(offset))
                // A function or a method a string names as a callable --
                // `function_exists('Illuminate\Support\enum_value')`, `[$repo, 'find']`,
                // `'App\Repo::find'`. Each is in its declaration's list of uses, so the rename
                // edits the very string the dialogue opens on; this list had learned about classes
                // in strings and nothing else, and refused to open where the rename worked.
                .chain(view.callable_named_by_a_string_at(offset))
                .map(|named| named.range()),
        )
        // A call's label, which names a parameter and is no name reference either.
        .chain(view.named_argument_at(offset).map(|(_, label)| label))
        // The alias a `use` clause writes, which is neither a name reference nor an access.
        .chain(
            view.trait_alias_at(offset)
                .map(|alias| view.trait_alias_occurrences(&alias))
                .unwrap_or_default(),
        )
        .chain(declared)
        .filter(|range| range.contains_inclusive(offset))
        .min_by_key(|range| range.len())
}

/// The part of a written reference that a rename replaces.
///
/// The last segment, wherever the reference is qualified. A member access and a declaration are one
/// segment already, so for those this is the range it was given.
///
/// A variable's `$` comes off for the same reason a namespace separator stays on: the editor shows
/// the range as the text to edit, and a person handed `$node` types `$other` -- which would make the
/// file say `$$other`. Offering `node` gets `other`, and the sigil is never in play.
pub fn rename_range(text: &str, range: TextRange) -> TextRange {
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    let Some(written) = text.get(start..end) else {
        return range;
    };

    if written.starts_with('$') {
        return TextRange::new(range.start() + TextSize::from(1), range.end());
    }

    match written.rfind('\\') {
        // Past the separator, which stays: only the name is being replaced.
        Some(at) => TextRange::new(
            range.start() + TextSize::try_from(at + 1).unwrap_or_default(),
            range.end(),
        ),
        None => range,
    }
}

/// Whether a new name is spelled the way PHP spells an identifier.
///
/// The lexical rule and nothing else -- whether the *word* is one PHP lets a class be called is
/// [`is_reserved_word`], and the two are separate because they are not true of the same things:
/// `class` is a perfectly good variable name and can never be a class name.
///
/// Checked here rather than left to the editor, because a rejected rename is recoverable and a
/// half-applied one that leaves a project unparseable is not.
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    // PHP's own rule, byte for byte: a letter or underscore, then letters, digits, underscores. The
    // high range is allowed too, which is why this is not `is_ascii_alphabetic`.
    let leads = first.is_alphabetic() || first == '_' || first as u32 >= 0x80;
    leads && chars.all(|ch| ch.is_alphanumeric() || ch == '_' || ch as u32 >= 0x80)
}

/// Whether PHP reserves the word, so no class, interface, trait or enum may be called it.
///
/// [`is_valid_name`] says nothing about this and cannot: `class` is a name PHP's lexer is perfectly
/// happy with, and `class Class {}` is a parse error in every file the rename would have written it
/// into. Renaming a class to one of these produced four edits in four files and a project that no
/// longer parses -- the exact outcome that function's own doc-comment calls unrecoverable.
///
/// Case-insensitively, because PHP's keywords are: `CLASS` is the same parse error.
///
/// Only the class-like position. PHP has let a *method* be called almost any of these since 7.0,
/// and a variable always could, so refusing them everywhere would decline renames the language
/// accepts -- see [`crate::rename_refusal`], which is what asks this and knows what is being
/// renamed.
pub fn is_reserved_word(name: &str) -> bool {
    RESERVED.iter().any(|word| word.eq_ignore_ascii_case(name))
}

/// Every word PHP will not let a class-like be named.
///
/// Three groups, and all three are needed. The keywords, which are a parse error in the
/// declaration; the magic constants, which the lexer replaces before a declaration is even seen;
/// and the type names -- `int`, `never`, `iterable` -- which are reserved precisely so that a type
/// position can never be ambiguous about whether it names a class.
const RESERVED: &[&str] = &[
    "abstract",
    "and",
    "array",
    "as",
    "bool",
    "break",
    "callable",
    "case",
    "catch",
    "class",
    "clone",
    "const",
    "continue",
    "declare",
    "default",
    "die",
    "do",
    "echo",
    "else",
    "elseif",
    "empty",
    "enddeclare",
    "endfor",
    "endforeach",
    "endif",
    "endswitch",
    "endwhile",
    "enum",
    "eval",
    "exit",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "fn",
    "for",
    "foreach",
    "function",
    "global",
    "goto",
    "if",
    "implements",
    "include",
    "include_once",
    "instanceof",
    "insteadof",
    "int",
    "interface",
    "isset",
    "iterable",
    "list",
    "match",
    "mixed",
    "namespace",
    "never",
    "new",
    "null",
    "object",
    "or",
    "parent",
    "print",
    "private",
    "protected",
    "public",
    "readonly",
    "require",
    "require_once",
    "return",
    "self",
    "static",
    "string",
    "switch",
    "throw",
    "trait",
    "true",
    "try",
    "unset",
    "use",
    "var",
    "void",
    "while",
    "xor",
    "yield",
    "__CLASS__",
    "__DIR__",
    "__FILE__",
    "__FUNCTION__",
    "__LINE__",
    "__METHOD__",
    "__NAMESPACE__",
    "__TRAIT__",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn replaced(text: &str) -> &str {
        let whole = TextRange::new(
            TextSize::from(0),
            TextSize::try_from(text.len()).expect("a test fits"),
        );
        let range = rename_range(text, whole);
        &text[usize::from(range.start())..usize::from(range.end())]
    }

    #[test]
    fn only_the_last_segment_of_a_written_path_is_replaced() {
        assert_eq!(replaced("Repo"), "Repo");
        assert_eq!(replaced("App\\Repo"), "Repo");
        assert_eq!(replaced("App\\Nested\\Repo"), "Repo");
    }

    #[test]
    fn a_variables_sigil_is_not_part_of_its_name() {
        assert_eq!(replaced("$node"), "node");
        assert_eq!(replaced("node"), "node");
    }

    #[test]
    fn a_name_php_would_reject_is_refused() {
        assert!(is_valid_name("Repo"));
        assert!(is_valid_name("_repo2"));
        assert!(is_valid_name("Ünicode"));

        assert!(!is_valid_name(""));
        assert!(!is_valid_name("2repo"));
        assert!(!is_valid_name("App\\Repo"));
        assert!(!is_valid_name("has space"));
        assert!(!is_valid_name("$repo"));
    }

    #[test]
    fn a_word_php_reserves_is_spelled_like_a_name_and_is_not_one() {
        // The whole point: every one of these passes the lexical rule, and `class Class {}` is a
        // parse error. A rename that only checked the spelling wrote four of them into four files.
        for word in [
            "class",
            "function",
            "list",
            "int",
            "never",
            "self",
            "__CLASS__",
        ] {
            assert!(is_valid_name(word), "{word} is spelled like a name");
            assert!(is_reserved_word(word), "{word} is reserved");
        }

        // PHP's keywords are case-insensitive, so the same parse error wears any spelling.
        assert!(is_reserved_word("CLASS"));
        assert!(is_reserved_word("Enum"));

        assert!(!is_reserved_word("Repo"));
        assert!(!is_reserved_word("classy"));
        assert!(!is_reserved_word("Klass"));
        assert!(!is_reserved_word(""));
    }
}
