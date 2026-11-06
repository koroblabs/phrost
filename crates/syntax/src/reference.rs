//! What a `@see` value points at, and where in the value it starts.
//!
//! `@see` is not in mago's tag vocabulary, so it arrives as a tag name and a run of text. Which
//! prefix of that text is a reference is the one thing read by hand -- and it is read by two
//! layers that cannot see each other: lowering, which records a name reference so the tag is
//! navigable, and the token layer, which colours it so a reader can tell a reference from the
//! prose beside it. Two readers of one rule is how one of them comes to be missing what the other
//! has, so the rule lives here, below both.
//!
//! Whether the token names anything real is deliberately not judged. `@see Monica` and `@see i18n`
//! are prose written where a reference goes, and nothing separates them from `@see LoggerCore`
//! without resolving.

use phrost_model::TextRange;

/// Which table a `@see` reference would be looked up in.
///
/// Chosen from the spelling rather than left for a lookup to try each, because the tables are
/// separate and a name in the wrong one resolves to nothing while the right one holds the answer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReferenceKind {
    /// A class, interface, trait or enum.
    ClassLike,
    Function,
    /// A global constant, written the way PHP writes one.
    Constant,
}

/// The name a `@see` value points at, as a range within the value.
///
/// The range covers the *name only*, never the `::member()` after it. Landing goto on the class
/// from a cursor on `id()` would be an answer to a question nobody asked; answering nothing there
/// is the honest half until a docblock can point at a member.
pub fn see_reference(value: &str) -> Option<(TextRange, ReferenceKind)> {
    let at = value.len() - value.trim_start().len();
    let token = value[at..].split_whitespace().next()?;
    // A URL is the other thing this tag holds -- 2 358 of one framework's own -- and `//` is not a
    // name, so the scheme is what has to be recognised before the separator confuses it.
    if token.contains("://") {
        return None;
    }

    // `Foo::bar()` names the class; `bar()` with no `::` names a function. The parentheses are
    // written on both and belong to neither name.
    let (path, kind) = match token.split_once("::") {
        Some((class, _)) => (class, ReferenceKind::ClassLike),
        None => match token.strip_suffix("()") {
            Some(function) => (function, ReferenceKind::Function),
            // PHP's own spelling for a global constant. A class named `JSON_ERROR_NONE` is not a
            // thing anybody writes; a constant referred to from a docblock is, 35 times on one
            // checkout.
            None if spelled_like_a_constant(token) => (token, ReferenceKind::Constant),
            None => (token, ReferenceKind::ClassLike),
        },
    };
    let path = path.strip_suffix("()").unwrap_or(path);
    if !spelled_like_a_name(path) {
        return None;
    }

    let start = u32::try_from(at).ok()?;
    let length = u32::try_from(path.len()).ok()?;
    Some((TextRange::at(start.into(), length.into()), kind))
}

/// Whether the token is written the way PHP writes a global constant.
///
/// Upper case with an underscore in it. The underscore is what keeps a one-word shout -- `TRUE`,
/// or a class somebody chose to name `URL` -- out of the constant table, where it would resolve to
/// nothing while the class table had the answer.
fn spelled_like_a_constant(token: &str) -> bool {
    token.contains('_')
        && !token.contains('\\')
        && token
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

/// Whether the token could be a name at all, qualified or not.
///
/// PHP's own rule per segment, so a trailing comma, a sentence's full stop, or the `$var` of a
/// `@see` written about a variable are all rejected before they reach a table.
fn spelled_like_a_name(path: &str) -> bool {
    let body = path.strip_prefix('\\').unwrap_or(path);
    !body.is_empty()
        && body.split('\\').all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|first| first.is_alphabetic() || first == '_' || first as u32 >= 0x80)
                && chars.all(|ch| ch.is_alphanumeric() || ch == '_' || ch as u32 >= 0x80)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(value: &str) -> Option<(&str, usize, ReferenceKind)> {
        see_reference(value).map(|(range, kind)| {
            (
                &value[usize::from(range.start())..usize::from(range.end())],
                usize::from(range.start()),
                kind,
            )
        })
    }

    #[test]
    fn the_reference_is_the_first_token_and_the_rest_is_a_description() {
        assert_eq!(
            read(" \\App\\Repo the repository"),
            Some(("\\App\\Repo", 1, ReferenceKind::ClassLike))
        );
        assert_eq!(read("Repo"), Some(("Repo", 0, ReferenceKind::ClassLike)));
    }

    #[test]
    fn a_member_reference_names_the_class_and_stops_before_the_member() {
        assert_eq!(
            read("\\App\\Repo::find()"),
            Some(("\\App\\Repo", 0, ReferenceKind::ClassLike))
        );
    }

    #[test]
    fn a_bare_call_names_a_function() {
        assert_eq!(
            read("hook_form_alter()"),
            Some(("hook_form_alter", 0, ReferenceKind::Function))
        );
    }

    #[test]
    fn a_shouted_name_with_an_underscore_is_a_constant() {
        assert_eq!(
            read("JSON_ERROR_NONE"),
            Some(("JSON_ERROR_NONE", 0, ReferenceKind::Constant))
        );
        // Without one it stays a class-like name, which is where a one-word shout resolves.
        assert_eq!(read("URL"), Some(("URL", 0, ReferenceKind::ClassLike)));
    }

    /// A qualified name is class-like however it is cased: the constant rule is about a bare
    /// shout, and a namespace separator says the writer meant a path.
    #[test]
    fn a_qualified_shout_is_still_a_path() {
        assert_eq!(
            read("\\App\\HTTP_CLIENT"),
            Some(("\\App\\HTTP_CLIENT", 0, ReferenceKind::ClassLike))
        );
    }

    #[test]
    fn what_is_not_spelled_like_a_name_is_not_one() {
        assert_eq!(read("https://www.drupal.org/node/1884796"), None);
        assert_eq!(read("http://php.net/manual/function.syslog.php"), None);
        assert_eq!(read("$rows the rows"), None);
        assert_eq!(read(""), None);
        assert_eq!(read("   "), None);
        // A sentence that begins where a reference would. Nothing separates this from a real class
        // name without resolving, so it is emitted and resolves to nothing -- but a token PHP
        // could not spell is refused here rather than interned.
        assert_eq!(read("2things"), None);
    }
}
